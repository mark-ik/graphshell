use super::*;
use crate::util::NoteAddress;
use euclid::default::Point2D;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use tempfile::TempDir;
use uuid::Uuid;

/// Create a unique RendererId for testing.
fn test_webview_id() -> RendererId {
    #[cfg(not(target_os = "ios"))]
    {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(42));
                cell.set(true);
            }
        });
        servo::WebViewId::new(base::id::PainterId::next())
    }
    #[cfg(target_os = "ios")]
    {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        RendererId(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

#[test]
fn create_note_for_node_creates_record_and_queues_note_open() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node_key = app.add_node_and_sync(
        "https://example.com/article".to_string(),
        Point2D::new(0.0, 0.0),
    );
    if let Some(node) = app.workspace.domain.graph.get_node_mut(node_key) {
        node.title = "Example Article".to_string();
    }

    let note_id = app
        .create_note_for_node(node_key, None)
        .expect("note should be created for an existing node");
    let note = app.note_record(note_id).expect("note record should exist");

    assert_eq!(note.title, "Note for Example Article");
    assert_eq!(note.linked_node, Some(node_key));
    assert_eq!(
        note.source_url.as_deref(),
        Some("https://example.com/article")
    );
    assert_eq!(app.take_pending_open_note_request(), Some(note_id));
    assert_eq!(
        app.take_pending_open_node_request(),
        Some(PendingNodeOpenRequest {
            key: node_key,
            mode: PendingTileOpenMode::SplitHorizontal,
        })
    );
}

#[test]
fn resolve_note_route_parses_note_url() {
    let note_id = NoteId::new();
    let note_url = NoteAddress::note(note_id.as_uuid().to_string()).to_string();

    assert_eq!(
        GraphBrowserApp::resolve_note_route(&note_url),
        Some(note_id)
    );
}

#[test]
fn resolve_note_route_rejects_invalid_note_url() {
    let note_url = "notes://not-a-uuid";
    assert_eq!(GraphBrowserApp::resolve_note_route(note_url), None);
}

#[test]
fn request_open_note_by_id_queues_note_open() {
    let mut app = GraphBrowserApp::new_for_testing();
    let note_id = NoteId::new();

    app.request_open_note_by_id(note_id);

    assert_eq!(app.take_pending_open_note_request(), Some(note_id));
}

#[test]
fn queued_open_requests_support_peek_before_take() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = NodeKey::new(7);

    app.request_open_node_tile_mode(key, PendingTileOpenMode::Tab);
    assert_eq!(
        app.pending_open_node_request(),
        Some(PendingNodeOpenRequest {
            key,
            mode: PendingTileOpenMode::Tab,
        })
    );
    assert_eq!(
        app.take_pending_open_node_request(),
        Some(PendingNodeOpenRequest {
            key,
            mode: PendingTileOpenMode::Tab,
        })
    );

    app.request_open_connected_from(
        key,
        PendingTileOpenMode::SplitHorizontal,
        PendingConnectedOpenScope::Connected,
    );
    assert_eq!(
        app.pending_open_connected_from(),
        Some((
            key,
            PendingTileOpenMode::SplitHorizontal,
            PendingConnectedOpenScope::Connected,
        ))
    );
    assert_eq!(
        app.take_pending_open_connected_from(),
        Some((
            key,
            PendingTileOpenMode::SplitHorizontal,
            PendingConnectedOpenScope::Connected,
        ))
    );
}

#[test]
fn queued_frame_import_requests_replace_previous_values() {
    let mut app = GraphBrowserApp::new_for_testing();
    let first = NodeKey::new(1);
    let second = NodeKey::new(2);

    app.request_add_node_to_frame(first, "alpha");
    app.request_add_node_to_frame(second, "beta");
    assert_eq!(
        app.take_pending_add_node_to_frame(),
        Some((second, "beta".to_string()))
    );

    app.request_add_connected_to_frame(vec![first], "alpha");
    app.request_add_connected_to_frame(vec![second], "beta");
    assert_eq!(
        app.take_pending_add_connected_to_frame(),
        Some((vec![second], "beta".to_string()))
    );

    app.request_add_exact_nodes_to_frame(vec![first], "alpha");
    app.request_add_exact_nodes_to_frame(vec![second], "beta");
    assert_eq!(
        app.take_pending_add_exact_to_frame(),
        Some((vec![second], "beta".to_string()))
    );
}

#[test]
fn removing_nodes_sanitizes_queued_frame_import_requests() {
    let mut app = GraphBrowserApp::new_for_testing();
    let kept = app.add_node_and_sync(
        "https://example.com/kept".to_string(),
        Point2D::new(0.0, 0.0),
    );
    let removed = app.add_node_and_sync(
        "https://example.com/removed".to_string(),
        Point2D::new(10.0, 0.0),
    );

    app.request_add_node_to_frame(removed, "stale-node");
    app.request_add_connected_to_frame(vec![removed, kept], "mixed-connected");
    app.request_add_exact_nodes_to_frame(vec![removed], "stale-exact");

    app.select_node(removed, false);
    app.remove_selected_nodes();

    assert_eq!(app.take_pending_add_node_to_frame(), None);
    assert_eq!(
        app.take_pending_add_connected_to_frame(),
        Some((vec![kept], "mixed-connected".to_string()))
    );
    assert_eq!(app.take_pending_add_exact_to_frame(), None);
}

#[test]
fn queued_tool_surface_return_target_supports_replace_peek_and_take() {
    let mut app = GraphBrowserApp::new_for_testing();
    let first = ToolSurfaceReturnTarget::Graph(GraphViewId::new());
    let second = ToolSurfaceReturnTarget::Node(NodeKey::new(42));

    app.set_pending_tool_surface_return_target(Some(first.clone()));
    app.set_pending_tool_surface_return_target(Some(second.clone()));

    assert_eq!(
        app.pending_tool_surface_return_target(),
        Some(second.clone())
    );
    assert_eq!(app.take_pending_tool_surface_return_target(), Some(second));
    assert!(app.pending_tool_surface_return_target().is_none());

    app.set_pending_tool_surface_return_target(Some(first));
    app.set_pending_tool_surface_return_target(None);
    assert!(app.take_pending_tool_surface_return_target().is_none());
}

#[test]
fn queued_command_surface_return_target_supports_replace_peek_and_take() {
    let mut app = GraphBrowserApp::new_for_testing();
    let first = ToolSurfaceReturnTarget::Graph(GraphViewId::new());
    let second = ToolSurfaceReturnTarget::Node(NodeKey::new(42));

    app.set_pending_command_surface_return_target(Some(first.clone()));
    app.set_pending_command_surface_return_target(Some(second.clone()));

    assert_eq!(
        app.pending_command_surface_return_target(),
        Some(second.clone())
    );
    assert_eq!(
        app.take_pending_command_surface_return_target(),
        Some(second)
    );
    assert!(app.pending_command_surface_return_target().is_none());

    app.set_pending_command_surface_return_target(Some(first));
    app.set_pending_command_surface_return_target(None);
    assert!(app.take_pending_command_surface_return_target().is_none());
}

#[test]
fn queued_transient_surface_return_target_supports_replace_peek_and_take() {
    let mut app = GraphBrowserApp::new_for_testing();
    let first = ToolSurfaceReturnTarget::Graph(GraphViewId::new());
    let second = ToolSurfaceReturnTarget::Node(NodeKey::new(42));

    app.set_pending_transient_surface_return_target(Some(first.clone()));
    app.set_pending_transient_surface_return_target(Some(second.clone()));

    assert_eq!(
        app.pending_transient_surface_return_target(),
        Some(second.clone())
    );
    assert_eq!(
        app.take_pending_transient_surface_return_target(),
        Some(second)
    );
    assert!(app.pending_transient_surface_return_target().is_none());

    app.set_pending_transient_surface_return_target(Some(first));
    app.set_pending_transient_surface_return_target(None);
    assert!(app.take_pending_transient_surface_return_target().is_none());
}

#[test]
fn test_select_node_marks_selection_state() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node_key = app
        .workspace
        .domain
        .graph
        .add_node("test".to_string(), Point2D::new(100.0, 100.0));

    app.select_node(node_key, false);

    // Node should be selected
    assert!(app.focused_selection().contains(&node_key));
}

#[test]
fn test_per_view_selection_isolated_and_restored_on_focus_switch() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

    let node_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(10.0, 10.0));
    let node_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(20.0, 20.0));

    app.set_workspace_focused_view_with_transition(Some(view_a));
    app.select_node(node_a, false);

    app.set_workspace_focused_view_with_transition(Some(view_b));
    app.select_node(node_b, false);

    assert_eq!(app.get_single_selected_node_for_view(view_a), Some(node_a));
    assert_eq!(app.get_single_selected_node_for_view(view_b), Some(node_b));

    app.set_workspace_focused_view_with_transition(Some(view_a));
    assert_eq!(app.get_single_selected_node(), Some(node_a));

    app.set_workspace_focused_view_with_transition(Some(view_b));
    assert_eq!(app.get_single_selected_node(), Some(node_b));
}

#[test]
fn undo_snapshot_uses_focused_view_selection_as_active_selection() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));

    let canonical = app
        .workspace
        .domain
        .graph
        .add_node("canonical".to_string(), Point2D::new(0.0, 0.0));
    let stale = app
        .workspace
        .domain
        .graph
        .add_node("stale".to_string(), Point2D::new(10.0, 0.0));

    app.set_workspace_focused_view_with_transition(Some(view_id));
    app.select_node(canonical, false);

    app.workspace
        .graph_runtime
        .selection_by_scope
        .insert(SelectionScope::Unfocused, {
            let mut selection = SelectionState::new();
            selection.select(stale, false);
            selection
        });

    let snapshot = app.build_undo_redo_snapshot(None).expect("snapshot");
    assert_eq!(snapshot.active_selection.primary(), Some(canonical));
    assert!(snapshot.active_selection.contains(&canonical));
    assert!(!snapshot.active_selection.contains(&stale));
}

#[test]
fn test_request_fit_to_screen() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.clear_pending_camera_command();
    assert!(app.pending_camera_command().is_none());

    // Request fit to screen
    app.request_fit_to_screen();
    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
    assert_eq!(app.pending_camera_command_target(), Some(view_id));

    app.clear_pending_camera_command();
    assert!(app.pending_camera_command().is_none());
}

#[test]
fn test_request_fit_to_screen_falls_back_to_single_view_when_unfocused() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "OnlyView"));
    app.workspace.graph_runtime.focused_view = None;

    app.clear_pending_camera_command();
    assert!(app.pending_camera_command().is_none());

    app.request_fit_to_screen();

    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
    assert_eq!(app.pending_camera_command_target(), Some(view_id));
}

#[test]
fn test_request_fit_to_screen_without_focus_and_multiple_views_is_noop() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
    app.workspace.graph_runtime.focused_view = None;
    app.workspace.graph_runtime.graph_view_frames.clear();

    app.clear_pending_camera_command();
    app.request_fit_to_screen();

    assert!(app.pending_camera_command().is_none());
    assert!(app.pending_camera_command_target().is_none());
}

#[test]
fn test_request_fit_to_screen_without_focus_targets_single_rendered_view() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
    app.workspace.graph_runtime.focused_view = None;
    app.workspace.graph_runtime.graph_view_frames.clear();
    app.workspace.graph_runtime.graph_view_frames.insert(
        view_b,
        GraphViewFrame {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        },
    );

    app.clear_pending_camera_command();
    app.request_fit_to_screen();

    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
    assert_eq!(app.pending_camera_command_target(), Some(view_b));
}

#[test]
fn test_toggle_camera_fit_locks_request_fit() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    app.clear_pending_camera_command();

    app.apply_reducer_intents([
        GraphIntent::ToggleCameraPositionFitLock,
        GraphIntent::ToggleCameraZoomFitLock,
    ]);

    assert!(app.camera_fit_locked());
    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
    assert_eq!(app.pending_camera_command_target(), Some(view_id));
}

#[test]
fn test_camera_locks_are_scoped_per_graph_view() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

    app.workspace.graph_runtime.focused_view = Some(view_a);
    app.set_camera_fit_locked(true);
    assert!(app.camera_fit_locked());

    app.workspace.graph_runtime.focused_view = Some(view_b);
    assert!(!app.camera_position_fit_locked());
    assert!(!app.camera_zoom_fit_locked());

    app.set_camera_position_fit_locked(true);
    assert!(app.camera_position_fit_locked());
    assert!(!app.camera_zoom_fit_locked());

    app.workspace.graph_runtime.focused_view = Some(view_a);
    assert!(app.camera_position_fit_locked());
    assert!(app.camera_zoom_fit_locked());
}

#[test]
fn test_unlock_camera_fit_lock_clears_pending_fit_and_restores_zoom_requests() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.set_camera_fit_locked(true);
    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));

    app.set_camera_fit_locked(false);
    assert!(!app.camera_fit_locked());
    assert!(app.pending_camera_command().is_none());

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );
}

#[test]
fn test_zoom_intents_noop_when_camera_fit_lock_enabled() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.set_camera_fit_locked(true);
    app.clear_pending_camera_command();

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
    assert_eq!(app.take_pending_keyboard_zoom_request(view_id), None);
    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));

    app.clear_pending_camera_command();
    app.workspace
        .graph_runtime
        .views
        .get_mut(&view_id)
        .unwrap()
        .camera
        .current_zoom = 2.0;
    app.apply_reducer_intents([GraphIntent::SetZoom { zoom: 0.25 }]);
    assert_eq!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .camera
            .current_zoom,
        2.0
    );
}

#[test]
fn test_position_fit_lock_does_not_block_manual_zoom() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.set_camera_position_fit_locked(true);
    app.set_camera_zoom_fit_locked(false);
    app.clear_pending_camera_command();

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );
}

#[test]
fn test_zoom_fit_lock_does_not_block_manual_pan_reheat_path() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    app.workspace.graph_runtime.physics.base.is_running = false;

    app.set_camera_position_fit_locked(false);
    app.set_camera_zoom_fit_locked(true);

    app.set_interacting(true);
    app.set_interacting(false);

    assert!(app.workspace.graph_runtime.physics.base.is_running);
    assert_eq!(
        app.workspace.graph_runtime.drag_release_frames_remaining,
        10
    );
}

#[test]
fn test_zoom_intents_queue_keyboard_zoom_requests() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );
    assert_eq!(app.take_pending_keyboard_zoom_request(view_id), None);

    app.apply_reducer_intents([GraphIntent::RequestZoomOut]);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::Out)
    );

    app.apply_reducer_intents([GraphIntent::RequestZoomReset]);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::Reset)
    );
}

#[test]
fn test_zoom_intent_targets_single_view_without_focus() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "OnlyView"));
    app.workspace.graph_runtime.focused_view = None;

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);

    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );
}

#[test]
fn test_restore_pending_keyboard_zoom_request_requeues_for_retry() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "RetryView"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
    let consumed = app.take_pending_keyboard_zoom_request(view_id);
    assert_eq!(consumed, Some(KeyboardZoomRequest::In));
    assert_eq!(app.take_pending_keyboard_zoom_request(view_id), None);

    app.restore_pending_keyboard_zoom_request(view_id, KeyboardZoomRequest::In);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );
}

#[test]
fn queued_keyboard_zoom_request_replaces_previous_target() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

    app.restore_pending_keyboard_zoom_request(view_a, KeyboardZoomRequest::In);
    app.restore_pending_keyboard_zoom_request(view_b, KeyboardZoomRequest::Out);

    assert_eq!(app.take_pending_keyboard_zoom_request(view_a), None);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_b),
        Some(KeyboardZoomRequest::Out)
    );
    assert_eq!(app.take_pending_keyboard_zoom_request(view_b), None);
}

#[test]
fn test_zoom_intent_without_focus_and_multiple_views_is_noop() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
    app.workspace.graph_runtime.focused_view = None;

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);

    assert_eq!(app.take_pending_keyboard_zoom_request(view_a), None);
    assert_eq!(app.take_pending_keyboard_zoom_request(view_b), None);
}

#[test]
fn test_zoom_intent_without_focus_targets_single_rendered_view() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
    app.workspace.graph_runtime.focused_view = None;
    app.workspace.graph_runtime.graph_view_frames.clear();
    app.workspace.graph_runtime.graph_view_frames.insert(
        view_b,
        GraphViewFrame {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        },
    );

    app.apply_reducer_intents([GraphIntent::RequestZoomIn]);

    assert_eq!(app.take_pending_keyboard_zoom_request(view_a), None);
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_b),
        Some(KeyboardZoomRequest::In)
    );
}

#[test]
fn test_zoom_to_selected_falls_back_to_fit_when_selection_empty() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    assert!(app.focused_selection().is_empty());
    app.clear_pending_camera_command();
    assert!(app.pending_camera_command().is_none());

    app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
    assert_eq!(app.pending_camera_command_target(), Some(view_id));
}

#[test]
fn test_zoom_to_selected_falls_back_to_fit_when_single_selected() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    let key = app
        .workspace
        .domain
        .graph
        .add_node("test".to_string(), Point2D::new(0.0, 0.0));
    app.select_node(key, false);
    app.clear_pending_camera_command();
    assert!(app.pending_camera_command().is_none());

    app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

    assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
    assert_eq!(app.pending_camera_command_target(), Some(view_id));
}

#[test]
fn test_zoom_to_selected_sets_pending_when_multi_selected() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    let key_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 50.0));
    app.select_node(key_a, false);
    app.select_node(key_b, true);
    assert_eq!(app.focused_selection().len(), 2);
    app.clear_pending_camera_command();
    assert!(app.pending_camera_command().is_none());

    app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

    assert_eq!(
        app.pending_camera_command(),
        Some(CameraCommand::FitSelection)
    );
    assert_eq!(app.pending_camera_command_target(), Some(view_id));
}

#[test]
fn test_zoom_to_selected_without_focus_and_multiple_views_is_noop() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
    app.workspace.graph_runtime.focused_view = None;
    let key_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 50.0));
    app.select_node(key_a, false);
    app.select_node(key_b, true);
    app.clear_pending_camera_command();

    app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

    assert!(app.pending_camera_command().is_none());
    assert!(app.pending_camera_command_target().is_none());
}

#[test]
fn test_request_camera_command_for_view_rejects_stale_target() {
    let mut app = GraphBrowserApp::new_for_testing();
    let stale_target = GraphViewId::new();
    app.clear_pending_camera_command();

    app.request_camera_command_for_view(Some(stale_target), CameraCommand::Fit);

    assert!(app.pending_camera_command().is_none());
    assert!(app.pending_camera_command_target_raw().is_none());
    assert!(app.pending_camera_command_target().is_none());
}

#[test]
fn test_request_camera_command_for_view_accepts_valid_target() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.clear_pending_camera_command();

    app.request_camera_command_for_view(Some(view_id), CameraCommand::FitSelection);

    assert_eq!(
        app.pending_camera_command(),
        Some(CameraCommand::FitSelection)
    );
    assert_eq!(app.pending_camera_command_target_raw(), Some(view_id));
    assert_eq!(app.pending_camera_command_target(), Some(view_id));
}

#[test]
fn queued_camera_command_replaces_previous_target() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

    app.request_camera_command_for_view(Some(view_a), CameraCommand::Fit);
    app.request_camera_command_for_view(Some(view_b), CameraCommand::FitSelection);

    assert_eq!(
        app.pending_camera_command(),
        Some(CameraCommand::FitSelection)
    );
    assert_eq!(app.pending_camera_command_target_raw(), Some(view_b));
    assert_eq!(app.pending_camera_command_target(), Some(view_b));
}

#[test]
fn test_frame_only_reducer_handles_zoom_and_selection_intents() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    let key_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 50.0));

    assert!(app.apply_workspace_only_intent(&GraphIntent::RequestZoomIn));
    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );

    assert!(
        app.apply_workspace_only_intent(&GraphIntent::UpdateSelection {
            keys: vec![key_a, key_b],
            mode: SelectionUpdateMode::Replace,
        })
    );
    assert_eq!(app.focused_selection().len(), 2);
    assert_eq!(app.focused_selection().primary(), Some(key_b));
}

#[test]
fn test_pending_wheel_zoom_delta_is_scoped_to_target_view() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();

    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

    app.queue_pending_wheel_zoom_delta(view_a, 32.0, Some((100.0, 120.0)));
    assert_eq!(app.pending_wheel_zoom_delta(view_a), 32.0);
    assert_eq!(app.pending_wheel_zoom_delta(view_b), 0.0);
    assert_eq!(
        app.pending_wheel_zoom_anchor_screen(view_a),
        Some((100.0, 120.0))
    );
    assert_eq!(app.pending_wheel_zoom_anchor_screen(view_b), None);

    app.queue_pending_wheel_zoom_delta(view_b, -12.0, Some((300.0, 240.0)));
    assert_eq!(app.pending_wheel_zoom_delta(view_a), 0.0);
    assert_eq!(app.pending_wheel_zoom_delta(view_b), -12.0);
    assert_eq!(app.pending_wheel_zoom_anchor_screen(view_a), None);
    assert_eq!(
        app.pending_wheel_zoom_anchor_screen(view_b),
        Some((300.0, 240.0))
    );
}

#[test]
fn test_clear_pending_wheel_zoom_delta_clears_target() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view, GraphViewState::new_with_id(view, "A"));

    app.queue_pending_wheel_zoom_delta(view, 24.0, Some((150.0, 80.0)));
    assert_eq!(app.pending_wheel_zoom_delta(view), 24.0);
    assert_eq!(
        app.pending_wheel_zoom_anchor_screen(view),
        Some((150.0, 80.0))
    );

    app.clear_pending_wheel_zoom_delta();
    assert_eq!(app.pending_wheel_zoom_delta(view), 0.0);
    assert_eq!(app.pending_wheel_zoom_anchor_screen(view), None);
}

#[test]
fn test_pending_wheel_zoom_anchor_is_retained_when_followup_delta_has_no_pointer() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view = GraphViewId::new();

    app.workspace
        .graph_runtime
        .views
        .insert(view, GraphViewState::new_with_id(view, "A"));

    app.queue_pending_wheel_zoom_delta(view, 20.0, Some((40.0, 55.0)));
    app.queue_pending_wheel_zoom_delta(view, 10.0, None);

    assert_eq!(app.pending_wheel_zoom_delta(view), 30.0);
    assert_eq!(
        app.pending_wheel_zoom_anchor_screen(view),
        Some((40.0, 55.0))
    );
}

#[test]
fn test_pending_wheel_zoom_anchor_updates_when_new_pointer_is_provided() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view = GraphViewId::new();

    app.workspace
        .graph_runtime
        .views
        .insert(view, GraphViewState::new_with_id(view, "A"));

    app.queue_pending_wheel_zoom_delta(view, 15.0, Some((10.0, 20.0)));
    app.queue_pending_wheel_zoom_delta(view, 5.0, Some((90.0, 120.0)));

    assert_eq!(app.pending_wheel_zoom_delta(view), 20.0);
    assert_eq!(
        app.pending_wheel_zoom_anchor_screen(view),
        Some((90.0, 120.0))
    );
}

#[test]
fn test_pending_wheel_zoom_anchor_clears_when_target_view_changes_without_anchor() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_a = GraphViewId::new();
    let view_b = GraphViewId::new();

    app.workspace
        .graph_runtime
        .views
        .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
    app.workspace
        .graph_runtime
        .views
        .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

    app.queue_pending_wheel_zoom_delta(view_a, 10.0, Some((25.0, 35.0)));
    assert_eq!(
        app.pending_wheel_zoom_anchor_screen(view_a),
        Some((25.0, 35.0))
    );

    app.queue_pending_wheel_zoom_delta(view_b, 6.0, None);

    assert_eq!(app.pending_wheel_zoom_delta(view_a), 0.0);
    assert_eq!(app.pending_wheel_zoom_anchor_screen(view_a), None);
    assert_eq!(app.pending_wheel_zoom_delta(view_b), 6.0);
    assert_eq!(app.pending_wheel_zoom_anchor_screen(view_b), None);
}

#[test]
fn test_frame_only_reducer_excludes_verse_side_effect_intents() {
    let mut app = GraphBrowserApp::new_for_testing();

    assert!(!app.apply_workspace_only_intent(&GraphIntent::SyncNow));
    assert!(
        !app.apply_workspace_only_intent(&GraphIntent::ForgetDevice {
            peer_id: "peer".to_string(),
        })
    );
    assert!(
        !app.apply_workspace_only_intent(&GraphIntent::RevokeWorkspaceAccess {
            peer_id: "peer".to_string(),
            workspace_id: "workspace".to_string(),
        })
    );
}

#[test]
fn graph_intent_category_helpers_expose_view_runtime_and_mutation_seams() {
    assert!(GraphIntent::RequestZoomIn.as_view_action().is_some());
    assert!(GraphIntent::RequestZoomIn.as_runtime_event().is_none());
    assert!(GraphIntent::RequestZoomIn.as_graph_mutation().is_none());

    assert!(GraphIntent::SyncNow.as_runtime_event().is_some());
    assert!(GraphIntent::SyncNow.as_view_action().is_none());
    assert!(GraphIntent::SyncNow.as_graph_mutation().is_none());

    assert!(
        GraphIntent::CreateNodeNearCenter
            .as_graph_mutation()
            .is_some()
    );
    assert!(GraphIntent::CreateNodeNearCenter.as_view_action().is_none());
    assert!(
        GraphIntent::CreateNodeNearCenter
            .as_runtime_event()
            .is_none()
    );
}

#[test]
fn app_command_queue_handles_non_snapshot_requests() {
    let mut app = GraphBrowserApp::new_for_testing();
    let note_id = NoteId::new();

    app.request_open_note_by_id(note_id);
    app.request_open_clip_by_id("clip-queue");
    app.request_prune_empty_workspaces();
    app.request_keep_latest_named_workspaces(3);
    app.request_switch_data_dir("C:/graphshell-data");

    assert_eq!(app.take_pending_open_note_request(), Some(note_id));
    assert_eq!(
        app.take_pending_open_clip_request().as_deref(),
        Some("clip-queue")
    );
    assert!(app.take_pending_prune_empty_workspaces());
    assert_eq!(app.take_pending_keep_latest_named_workspaces(), Some(3));
    assert_eq!(
        app.take_pending_switch_data_dir(),
        Some(PathBuf::from("C:/graphshell-data"))
    );
}

#[test]
fn apply_view_actions_dispatches_without_graph_intent_wrapper() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.apply_view_actions([ViewAction::RequestZoomIn]);

    assert_eq!(
        app.take_pending_keyboard_zoom_request(view_id),
        Some(KeyboardZoomRequest::In)
    );
}

#[test]
fn apply_runtime_events_dispatches_without_graph_intent_wrapper() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.workspace.domain.graph.add_node(
        "https://runtime.example".to_string(),
        Point2D::new(0.0, 0.0),
    );

    app.apply_runtime_events([RuntimeEvent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::Restore,
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
}

#[test]
fn contract_only_trusted_writers_call_graph_topology_mutators() {
    const FORBIDDEN_TOKENS: [&str; 11] = [
        "graph.add_node(",
        "graph.remove_node(",
        "graph.add_edge(",
        "graph.remove_edges(",
        "graph.inner.",
        "graph.get_node_mut(",
        ".add_node_and_sync(",
        ".add_edge_and_sync(",
        ".capture_undo_checkpoint(",
        ".perform_undo(",
        ".perform_redo(",
    ];
    const PERSISTENCE_DURABLE_ESCAPE_HATCH_TOKENS: [&str; 3] = [
        "graph.get_node_mut(",
        "graph.get_edge_mut(",
        "graph.update_node_url(",
    ];
    const RENDER_DURABLE_POSITION_ESCAPE_HATCH_TOKENS: [&str; 1] = ["graph.set_node_position("];
    const RENDER_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 2] =
        ["workspace.domain.graph.", "workspace\n        .graph"];
    const RENDER_PROJECTED_READ_ESCAPE_HATCH_TOKENS: [&str; 1] = ["node.position"];
    const PROJECTED_READ_ESCAPE_HATCH_TOKENS: [&str; 1] = ["node.position"];
    const WORKBENCH_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const LIFECYCLE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const ACTION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const GUI_FRAME_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const COMMAND_PALETTE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const GUI_ORCHESTRATION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
        ["workspace.domain.graph"];
    const TOOLBAR_OMNIBAR_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const THUMBNAIL_PIPELINE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
        ["workspace.domain.graph"];
    const GUI_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
    const PERSISTENCE_OPS_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];

    let persistence_runtime_only = include_str!("services/persistence/mod.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let webview_controller_runtime_only =
        include_str!("shell/desktop/lifecycle/webview_controller.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
    let webview_backpressure_runtime_only =
        include_str!("shell/desktop/lifecycle/webview_backpressure.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
    let lifecycle_reconcile_runtime_only =
        include_str!("shell/desktop/lifecycle/lifecycle_reconcile.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
    let render_runtime_only = include_str!("render/mod.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let action_registry_runtime_only = include_str!("shell/desktop/runtime/registries/action.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let runtime_registries_runtime_only = include_str!("shell/desktop/runtime/registries/mod.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let gui_runtime_only = include_str!("shell/desktop/ui/gui.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let gui_frame_runtime_only = include_str!("shell/desktop/ui/gui_frame.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let gui_orchestration_runtime_only = include_str!("shell/desktop/ui/gui_orchestration.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let persistence_ops_runtime_only = include_str!("shell/desktop/ui/persistence_ops.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let thumbnail_pipeline_runtime_only = include_str!("shell/desktop/ui/thumbnail_pipeline.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let command_palette_runtime_only = include_str!("render/command_palette.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let tile_behavior_runtime_only = include_str!("shell/desktop/workbench/tile_behavior.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let tile_runtime_runtime_only = include_str!("shell/desktop/workbench/tile_runtime.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let tile_invariants_runtime_only = include_str!("shell/desktop/workbench/tile_invariants.rs")
        .split("\n#[cfg(test)]")
        .next()
        .unwrap_or_default();

    let guarded_sources = [
        (
            "shell/desktop/host/running_app_state.rs",
            include_str!("shell/desktop/host/running_app_state.rs"),
        ),
        (
            "shell/desktop/host/window.rs",
            include_str!("shell/desktop/host/window.rs"),
        ),
        (
            "shell/desktop/lifecycle/lifecycle_reconcile.rs",
            include_str!("shell/desktop/lifecycle/lifecycle_reconcile.rs"),
        ),
        (
            "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
            webview_controller_runtime_only,
        ),
        (
            "shell/desktop/lifecycle/semantic_event_pipeline.rs",
            include_str!("shell/desktop/lifecycle/semantic_event_pipeline.rs"),
        ),
        (
            "shell/desktop/host/event_loop.rs",
            include_str!("shell/desktop/host/event_loop.rs"),
        ),
        (
            "shell/desktop/runtime/registries/action.rs (runtime section)",
            action_registry_runtime_only,
        ),
        (
            "shell/desktop/runtime/registries/mod.rs (runtime section)",
            runtime_registries_runtime_only,
        ),
        ("render/mod.rs (runtime section)", render_runtime_only),
        (
            "shell/desktop/ui/gui.rs (runtime section)",
            gui_runtime_only,
        ),
        (
            "shell/desktop/ui/gui_frame.rs (runtime section)",
            gui_frame_runtime_only,
        ),
        (
            "shell/desktop/ui/gui_orchestration.rs (runtime section)",
            gui_orchestration_runtime_only,
        ),
        (
            "services/persistence/mod.rs (runtime section)",
            persistence_runtime_only,
        ),
    ];

    for (path, source) in guarded_sources {
        for token in FORBIDDEN_TOKENS {
            assert!(
                !source.contains(token),
                "trusted-writer boundary violated in {path}: found '{token}'"
            );
        }
    }

    for token in PERSISTENCE_DURABLE_ESCAPE_HATCH_TOKENS {
        assert!(
            !persistence_runtime_only.contains(token),
            "trusted-writer boundary violated in services/persistence/mod.rs (runtime section): found '{token}'"
        );
    }

    for token in RENDER_DURABLE_POSITION_ESCAPE_HATCH_TOKENS {
        assert!(
            !render_runtime_only.contains(token),
            "trusted-writer boundary violated in render/mod.rs (runtime section): found '{token}'"
        );
    }

    for token in RENDER_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
        assert!(
            !render_runtime_only.contains(token),
            "domain-state CLAT boundary violated in render/mod.rs (runtime section): found '{token}'"
        );
    }

    for token in RENDER_PROJECTED_READ_ESCAPE_HATCH_TOKENS {
        assert!(
            !render_runtime_only.contains(token),
            "projected-position boundary violated in render/mod.rs (runtime section): found '{token}'"
        );
    }

    for (path, source) in [
        (
            "graph_app.rs",
            include_str!("graph_app.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default(),
        ),
        (
            "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
            webview_controller_runtime_only,
        ),
        (
            "shell/desktop/runtime/registries/action.rs (runtime section)",
            action_registry_runtime_only,
        ),
        (
            "shell/desktop/ui/toolbar/toolbar_omnibar.rs",
            include_str!("shell/desktop/ui/toolbar/toolbar_omnibar.rs"),
        ),
    ] {
        for token in PROJECTED_READ_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "projected-position boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [
        (
            "shell/desktop/lifecycle/lifecycle_reconcile.rs (runtime section)",
            lifecycle_reconcile_runtime_only,
        ),
        (
            "shell/desktop/lifecycle/webview_backpressure.rs (runtime section)",
            webview_backpressure_runtime_only,
        ),
        (
            "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
            webview_controller_runtime_only,
        ),
        (
            "shell/desktop/workbench/tile_behavior.rs (runtime section)",
            tile_behavior_runtime_only,
        ),
        (
            "shell/desktop/workbench/tile_runtime.rs (runtime section)",
            tile_runtime_runtime_only,
        ),
        (
            "shell/desktop/workbench/tile_invariants.rs (runtime section)",
            tile_invariants_runtime_only,
        ),
        (
            "shell/desktop/workbench/tile_view_ops.rs",
            include_str!("shell/desktop/workbench/tile_view_ops.rs"),
        ),
        (
            "shell/desktop/workbench/ux_tree.rs",
            include_str!("shell/desktop/workbench/ux_tree.rs"),
        ),
        (
            "shell/desktop/workbench/pane_model.rs",
            include_str!("shell/desktop/workbench/pane_model.rs"),
        ),
    ] {
        for token in WORKBENCH_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [
        (
            "shell/desktop/lifecycle/lifecycle_reconcile.rs (runtime section)",
            lifecycle_reconcile_runtime_only,
        ),
        (
            "shell/desktop/lifecycle/webview_backpressure.rs (runtime section)",
            webview_backpressure_runtime_only,
        ),
        (
            "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
            webview_controller_runtime_only,
        ),
    ] {
        for token in LIFECYCLE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/runtime/registries/action.rs (runtime section)",
        action_registry_runtime_only,
    )] {
        for token in ACTION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/ui/gui_frame.rs (runtime section)",
        gui_frame_runtime_only,
    )] {
        for token in GUI_FRAME_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "render/command_palette.rs (runtime section)",
        command_palette_runtime_only,
    )] {
        for token in COMMAND_PALETTE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/ui/gui_orchestration.rs (runtime section)",
        gui_orchestration_runtime_only,
    )] {
        for token in GUI_ORCHESTRATION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/ui/toolbar/toolbar_omnibar.rs",
        include_str!("shell/desktop/ui/toolbar/toolbar_omnibar.rs"),
    )] {
        for token in TOOLBAR_OMNIBAR_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/ui/thumbnail_pipeline.rs (runtime section)",
        thumbnail_pipeline_runtime_only,
    )] {
        for token in THUMBNAIL_PIPELINE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/ui/gui.rs (runtime section)",
        gui_runtime_only,
    )] {
        for token in GUI_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }

    for (path, source) in [(
        "shell/desktop/ui/persistence_ops.rs (runtime section)",
        persistence_ops_runtime_only,
    )] {
        for token in PERSISTENCE_OPS_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !source.contains(token),
                "domain-state CLAT boundary violated in {path}: found '{token}'"
            );
        }
    }
}

#[test]
fn test_select_node_single() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("test".to_string(), Point2D::new(0.0, 0.0));

    app.select_node(key, false);

    assert_eq!(app.focused_selection().len(), 1);
    assert!(app.focused_selection().contains(&key));
}

#[test]
fn test_select_node_single_click_selected_toggles_off() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("test".to_string(), Point2D::new(0.0, 0.0));

    app.select_node(key, false);
    assert_eq!(app.focused_selection().primary(), Some(key));

    app.select_node(key, false);
    assert!(app.focused_selection().is_empty());
    assert_eq!(app.focused_selection().primary(), None);
}

#[test]
fn test_select_node_multi() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));

    app.select_node(key1, false);
    app.select_node(key2, true);

    assert_eq!(app.focused_selection().len(), 2);
    assert!(app.focused_selection().contains(&key1));
    assert!(app.focused_selection().contains(&key2));
}

#[test]
fn test_select_node_multi_click_selected_toggles_off() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));

    app.select_node(key1, false);
    app.select_node(key2, true);
    assert_eq!(app.focused_selection().len(), 2);
    assert_eq!(app.focused_selection().primary(), Some(key2));

    // Ctrl-click selected node toggles it off.
    app.select_node(key2, true);
    assert_eq!(app.focused_selection().len(), 1);
    assert!(app.focused_selection().contains(&key1));
    assert!(!app.focused_selection().contains(&key2));
    assert_eq!(app.focused_selection().primary(), Some(key1));
}

#[test]
fn test_select_node_multi_click_only_selected_clears_selection() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));

    app.select_node(key, false);
    assert_eq!(app.focused_selection().primary(), Some(key));

    // Ctrl-click selected single node toggles it off, clearing selection.
    app.select_node(key, true);
    assert!(app.focused_selection().is_empty());
    assert_eq!(app.focused_selection().primary(), None);
}

#[test]
fn test_select_node_intent_single_prewarms_cold_node() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );

    app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
}

#[test]
fn test_select_node_intent_toggle_off_does_not_prewarm() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);
    app.demote_node_to_cold(key);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );

    // Clicking the already-selected node toggles it off and should not re-promote.
    app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);

    assert!(app.focused_selection().is_empty());
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
}

#[test]
fn test_select_node_intent_multiselect_does_not_prewarm_cold_node() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(10.0, 0.0));

    app.apply_reducer_intents([GraphIntent::SelectNode {
        key: key1,
        multi_select: false,
    }]);
    app.demote_node_to_cold(key1);
    assert_eq!(
        app.workspace.domain.graph.get_node(key1).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
    assert_eq!(
        app.workspace.domain.graph.get_node(key2).unwrap().lifecycle,
        NodeLifecycle::Cold
    );

    app.apply_reducer_intents([GraphIntent::SelectNode {
        key: key2,
        multi_select: true,
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key2).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
}

#[test]
fn test_select_node_intent_does_not_prewarm_crashed_node() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let webview_id = test_webview_id();
    app.map_webview_to_node(webview_id, key);
    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id,
        reason: "boom".to_string(),
        has_backtrace: false,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
    assert!(app.runtime_crash_state_for_node(key).is_some());

    app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
}

#[test]
fn test_selection_revision_increments_on_change() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(1.0, 0.0));
    let rev0 = app.focused_selection().revision();

    app.select_node(key1, false);
    let rev1 = app.focused_selection().revision();
    assert!(rev1 > rev0);

    app.select_node(key1, false);
    let rev2 = app.focused_selection().revision();
    assert!(rev2 > rev1);
    assert!(app.focused_selection().is_empty());

    app.select_node(key2, true);
    let rev3 = app.focused_selection().revision();
    assert!(rev3 > rev2);

    app.select_node(key2, true);
    let rev4 = app.focused_selection().revision();
    assert!(rev4 > rev3);
}

#[test]
fn test_update_selection_replace_sets_exact_members() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app.add_node_and_sync("a".to_string(), Point2D::new(0.0, 0.0));
    let b = app.add_node_and_sync("b".to_string(), Point2D::new(10.0, 0.0));
    let c = app.add_node_and_sync("c".to_string(), Point2D::new(20.0, 0.0));
    app.select_node(a, false);

    app.apply_reducer_intents([GraphIntent::UpdateSelection {
        keys: vec![b, c],
        mode: SelectionUpdateMode::Replace,
    }]);

    assert_eq!(app.focused_selection().len(), 2);
    assert!(!app.focused_selection().contains(&a));
    assert!(app.focused_selection().contains(&b));
    assert!(app.focused_selection().contains(&c));
    assert_eq!(app.focused_selection().primary(), Some(c));
}

#[test]
fn test_update_selection_add_and_toggle() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app.add_node_and_sync("a".to_string(), Point2D::new(0.0, 0.0));
    let b = app.add_node_and_sync("b".to_string(), Point2D::new(10.0, 0.0));
    app.apply_reducer_intents([GraphIntent::UpdateSelection {
        keys: vec![a],
        mode: SelectionUpdateMode::Replace,
    }]);
    app.apply_reducer_intents([GraphIntent::UpdateSelection {
        keys: vec![b],
        mode: SelectionUpdateMode::Add,
    }]);
    assert!(app.focused_selection().contains(&a));
    assert!(app.focused_selection().contains(&b));
    assert_eq!(app.focused_selection().primary(), Some(b));

    app.apply_reducer_intents([GraphIntent::UpdateSelection {
        keys: vec![a],
        mode: SelectionUpdateMode::Toggle,
    }]);
    assert!(!app.focused_selection().contains(&a));
    assert!(app.focused_selection().contains(&b));
}

#[test]
fn test_intent_webview_created_links_parent_without_direct_selection_mutation() {
    let mut app = GraphBrowserApp::new_for_testing();
    let parent = app
        .workspace
        .domain
        .graph
        .add_node("https://parent.com".into(), Point2D::new(10.0, 20.0));
    let parent_wv = test_webview_id();
    let child_wv = test_webview_id();
    app.map_webview_to_node(parent_wv, parent);

    let edges_before = app.workspace.domain.graph.edge_count();
    app.apply_reducer_intents([GraphIntent::WebViewCreated {
        parent_webview_id: parent_wv,
        child_webview_id: child_wv,
        initial_url: Some("https://child.com".into()),
    }]);

    assert_eq!(app.workspace.domain.graph.edge_count(), edges_before + 1);
    let child = app.get_node_for_webview(child_wv).unwrap();
    assert_eq!(app.get_single_selected_node(), None);
    assert_eq!(
        app.workspace.domain.graph.get_node(child).unwrap().url(),
        "https://child.com"
    );
}

#[test]
fn test_intent_webview_created_places_child_near_parent() {
    let mut app = GraphBrowserApp::new_for_testing();
    let parent = app
        .workspace
        .domain
        .graph
        .add_node("https://parent.com".into(), Point2D::new(10.0, 20.0));
    let parent_wv = test_webview_id();
    let child_wv = test_webview_id();
    app.map_webview_to_node(parent_wv, parent);

    app.apply_reducer_intents([GraphIntent::WebViewCreated {
        parent_webview_id: parent_wv,
        child_webview_id: child_wv,
        initial_url: Some("https://child.com".into()),
    }]);

    let child = app.get_node_for_webview(child_wv).unwrap();
    let child_pos = app
        .workspace
        .domain
        .graph
        .get_node(child)
        .unwrap()
        .projected_position();
    // Child should be placed near the parent (not at fallback center 400, 300).
    // The base offset is (+140, +80) plus jitter in [-50, +50].
    // So x is in [100, 200] and y is in [50, 150] relative to parent at (10, 20).
    assert!(child_pos.x >= 10.0 + 140.0 - 50.0 && child_pos.x <= 10.0 + 140.0 + 50.0);
    assert!(child_pos.y >= 20.0 + 80.0 - 50.0 && child_pos.y <= 20.0 + 80.0 + 50.0);
}

#[test]
fn test_intent_webview_created_about_blank_uses_placeholder() {
    let mut app = GraphBrowserApp::new_for_testing();
    let child_wv = test_webview_id();

    app.apply_reducer_intents([GraphIntent::WebViewCreated {
        parent_webview_id: test_webview_id(),
        child_webview_id: child_wv,
        initial_url: Some("about:blank".into()),
    }]);

    let child = app.get_node_for_webview(child_wv).unwrap();
    assert!(
        app.workspace
            .domain
            .graph
            .get_node(child)
            .unwrap()
            .url()
            .starts_with("about:blank#")
    );
}

#[test]
fn test_intent_webview_url_changed_updates_existing_mapping() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://before.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);

    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://after.com".into(),
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().url(),
        "https://after.com"
    );
    assert_eq!(app.get_node_for_webview(wv), Some(key));
}

#[test]
fn test_webview_url_changed_appends_traversal_between_known_nodes() {
    // Navigating from a known node (a) to another known node (b) via WebViewUrlChanged
    // must append a traversal on the a→b edge. The prior URL must be captured BEFORE
    // update_node_url_and_log overwrites it; otherwise the traversal would be recorded
    // on the wrong edge (b→b self-loop) rather than the correct a→b edge.
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let b = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, a);

    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://b.com".into(),
    }]);

    let edge_key = app
        .workspace
        .domain
        .graph
        .find_edge_key(a, b)
        .expect("traversal edge from a to b should exist");
    let payload = app.workspace.domain.graph.get_edge(edge_key).unwrap();
    assert_eq!(payload.traversals().len(), 1);
    assert_eq!(payload.traversals()[0].trigger, NavigationTrigger::Unknown);
    // No self-loop on b — confirms prior URL was captured before mutation.
    assert!(app.workspace.domain.graph.find_edge_key(b, b).is_none());
}

#[test]
fn test_webview_url_changed_self_loop_navigation_does_not_append_traversal() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, a);

    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://a.com".into(),
    }]);

    let history_edge_count = app
        .workspace
        .domain
        .graph
        .edges()
        .filter(|e| e.edge_type == EdgeType::History)
        .count();
    assert_eq!(history_edge_count, 0);
}

#[test]
fn test_intent_webview_url_changed_ignores_unmapped_webview() {
    let mut app = GraphBrowserApp::new_for_testing();
    let wv = test_webview_id();
    let before = app.workspace.domain.graph.node_count();

    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://ignored.com".into(),
    }]);

    assert_eq!(app.workspace.domain.graph.node_count(), before);
    assert_eq!(app.get_node_for_webview(wv), None);
}

#[test]
fn test_intent_webview_history_changed_clamps_index() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec!["https://a.com".into(), "https://b.com".into()],
        current: 99,
    }]);

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.history_entries.len(), 2);
    assert_eq!(node.history_index, 1);
}

#[test]
fn test_intent_webview_scroll_changed_updates_node_session_scroll() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);

    app.apply_reducer_intents([GraphIntent::WebViewScrollChanged {
        webview_id: wv,
        scroll_x: 15.0,
        scroll_y: 320.0,
    }]);

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.session_scroll, Some((15.0, 320.0)));
}

#[test]
fn test_form_draft_restore_feature_flag_guarded() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));

    app.set_form_draft_capture_enabled_for_testing(false);
    app.apply_reducer_intents([GraphIntent::SetNodeFormDraft {
        key,
        form_draft: Some("draft text".to_string()),
    }]);
    assert_eq!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .session_form_draft,
        None
    );

    app.set_form_draft_capture_enabled_for_testing(true);
    app.apply_reducer_intents([GraphIntent::SetNodeFormDraft {
        key,
        form_draft: Some("draft text".to_string()),
    }]);
    assert_eq!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .session_form_draft,
        Some("draft text".to_string())
    );
}

#[test]
fn test_intent_webview_history_changed_adds_history_edge_on_back() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, to);
    if let Some(node) = app.workspace.domain.graph.get_node_mut(to) {
        node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
        node.history_index = 1;
    }

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec!["https://a.com".into(), "https://b.com".into()],
        current: 0,
    }]);

    let has_edge = app
        .workspace
        .domain
        .graph
        .edges()
        .any(|e| e.edge_type == EdgeType::History && e.from == to && e.to == from);
    assert!(has_edge);
}

#[test]
fn test_intent_webview_history_changed_does_not_add_edge_on_normal_navigation() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
        node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
        node.history_index = 1;
    }

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec![
            "https://a.com".into(),
            "https://b.com".into(),
            "https://c.com".into(),
        ],
        current: 2,
    }]);

    let history_edge_count = app
        .workspace
        .domain
        .graph
        .edges()
        .filter(|e| e.edge_type == EdgeType::History)
        .count();
    assert_eq!(history_edge_count, 0);
}

#[test]
fn test_intent_webview_history_changed_adds_history_edge_on_forward_same_list() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, from);
    if let Some(node) = app.workspace.domain.graph.get_node_mut(from) {
        node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
        node.history_index = 0;
    }

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec!["https://a.com".into(), "https://b.com".into()],
        current: 1,
    }]);

    let has_edge = app
        .workspace
        .domain
        .graph
        .edges()
        .any(|e| e.edge_type == EdgeType::History && e.from == from && e.to == to);
    assert!(has_edge);
}

#[test]
fn test_intent_webview_history_changed_appends_traversals_on_repeat_navigation() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let b = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, b);
    if let Some(node) = app.workspace.domain.graph.get_node_mut(b) {
        node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
        node.history_index = 1;
    }

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec!["https://a.com".into(), "https://b.com".into()],
        current: 0,
    }]);

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec!["https://a.com".into(), "https://b.com".into()],
        current: 1,
    }]);

    let back_edge_key = app
        .workspace
        .domain
        .graph
        .find_edge_key(b, a)
        .expect("back traversal edge");
    let back_payload = app.workspace.domain.graph.get_edge(back_edge_key).unwrap();
    assert_eq!(back_payload.traversals().len(), 1);
    assert_eq!(
        back_payload.traversals()[0].trigger,
        NavigationTrigger::Back
    );

    let forward_edge_key = app
        .workspace
        .domain
        .graph
        .find_edge_key(a, b)
        .expect("forward traversal edge");
    let forward_payload = app
        .workspace
        .domain
        .graph
        .get_edge(forward_edge_key)
        .unwrap();
    assert_eq!(forward_payload.traversals().len(), 1);
    assert_eq!(
        forward_payload.traversals()[0].trigger,
        NavigationTrigger::Forward
    );

    app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
        webview_id: wv,
        entries: vec!["https://a.com".into(), "https://b.com".into()],
        current: 0,
    }]);

    let back_payload = app.workspace.domain.graph.get_edge(back_edge_key).unwrap();
    assert_eq!(back_payload.traversals().len(), 2);
    assert_eq!(
        back_payload.traversals()[1].trigger,
        NavigationTrigger::Back
    );
}

#[test]
fn set_and_clear_highlighted_edge_do_not_append_traversal() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let b = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, a);

    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://b.com".into(),
    }]);

    let edge_key = app
        .workspace
        .domain
        .graph
        .find_edge_key(a, b)
        .expect("history traversal edge should exist");
    let before = app
        .workspace
        .domain
        .graph
        .get_edge(edge_key)
        .expect("edge payload")
        .traversals()
        .len();

    app.apply_reducer_intents([GraphIntent::SetHighlightedEdge { from: a, to: b }]);
    app.apply_reducer_intents([GraphIntent::ClearHighlightedEdge]);

    let after = app
        .workspace
        .domain
        .graph
        .get_edge(edge_key)
        .expect("edge payload")
        .traversals()
        .len();
    assert_eq!(before, after);
}

#[test]
fn history_health_summary_tracks_capture_status_and_append_failures() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));

    let before = app.history_health_summary();
    assert_eq!(
        before.capture_status,
        HistoryCaptureStatus::DegradedCaptureOnly
    );
    assert_eq!(before.recent_traversal_append_failures, 0);
    assert!(before.last_event_unix_ms.is_none());

    assert!(!app.push_history_traversal_and_sync(a, a, NavigationTrigger::Unknown));

    let after = app.history_health_summary();
    assert_eq!(
        after.capture_status,
        HistoryCaptureStatus::DegradedCaptureOnly
    );
    assert_eq!(after.recent_traversal_append_failures, 1);
    assert_eq!(
        after.recent_failure_reason_bucket.as_deref(),
        Some("self_loop")
    );
    assert!(
        after
            .last_error
            .as_deref()
            .is_some_and(|msg| msg.contains("self_loop"))
    );
    assert!(!after.preview_mode_active);
    assert!(!after.last_preview_isolation_violation);
    assert!(after.last_event_unix_ms.is_some());
}

#[test]
fn history_archive_counts_consistent_after_dissolution_and_clear() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());

    let a = app.add_node_and_sync("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let b = app.add_node_and_sync("https://b.com".to_string(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, a);
    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://b.com".into(),
    }]);

    let before = app.history_manager_archive_counts();
    assert_eq!(before.0, 0);
    assert_eq!(before.1, 0);

    app.apply_reducer_intents([GraphIntent::RemoveEdge {
        from: a,
        to: b,
        selector: crate::graph::RelationSelector::Family(crate::graph::EdgeFamily::Traversal),
    }]);

    let after_remove = app.history_manager_archive_counts();
    assert_eq!(after_remove.0, 0);
    assert!(after_remove.1 > 0);
    assert_eq!(
        app.history_manager_dissolved_entries(usize::MAX).len(),
        after_remove.1
    );

    app.apply_reducer_intents([GraphIntent::ClearHistoryDissolved]);
    let after_clear = app.history_manager_archive_counts();
    assert_eq!(after_clear.0, 0);
    assert_eq!(after_clear.1, 0);
}

#[test]
fn history_archive_auto_curation_keeps_latest_entries() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    let from = Uuid::new_v4().to_string();
    let to = Uuid::new_v4().to_string();

    {
        let store = app
            .services
            .persistence
            .as_mut()
            .expect("persistence store should exist");
        for i in 0..6u64 {
            let entry = crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: from.clone(),
                to_node_id: to.clone(),
                timestamp_ms: i,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
            };
            store
                .archive_append_traversal(&entry)
                .expect("archive traversal should succeed");
            store
                .archive_dissolved_traversal(&entry)
                .expect("archive dissolved should succeed");
        }
    }

    app.apply_reducer_intents([
        GraphIntent::AutoCurateHistoryTimeline { keep_latest: 2 },
        GraphIntent::AutoCurateHistoryDissolved { keep_latest: 3 },
    ]);

    let (timeline_count, dissolved_count) = app.history_manager_archive_counts();
    assert_eq!(timeline_count, 2);
    assert_eq!(dissolved_count, 3);

    let timeline = app.history_manager_timeline_entries(usize::MAX);
    assert_eq!(timeline.len(), 2);
    match &timeline[0] {
        crate::services::persistence::types::LogEntry::AppendTraversal { timestamp_ms, .. } => {
            assert_eq!(*timestamp_ms, 5)
        }
        _ => panic!("expected traversal entry"),
    }
}

#[test]
fn history_runtime_clear_operations_only_touch_requested_archive() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let entry = crate::services::persistence::types::LogEntry::AppendTraversal {
        from_node_id: from.to_string(),
        to_node_id: to.to_string(),
        timestamp_ms: 10,
        trigger: crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
    };

    {
        let store = app
            .services
            .persistence
            .as_mut()
            .expect("persistence store should exist");
        store
            .archive_append_traversal(&entry)
            .expect("timeline archive write should succeed");
        store
            .archive_dissolved_traversal(&entry)
            .expect("dissolved archive write should succeed");
    }

    assert_eq!(app.history_manager_archive_counts(), (1, 1));

    app.apply_reducer_intents([GraphIntent::ClearHistoryTimeline]);
    assert_eq!(
        app.history_manager_archive_counts(),
        (0, 1),
        "clearing timeline should preserve dissolved archive entries"
    );

    app.apply_reducer_intents([GraphIntent::ClearHistoryDissolved]);
    assert_eq!(
        app.history_manager_archive_counts(),
        (0, 0),
        "clearing dissolved should only clear dissolved archive entries"
    );
}

#[test]
fn history_runtime_auto_curate_preserves_newest_entries_for_each_archive() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());

    {
        let store = app
            .services
            .persistence
            .as_mut()
            .expect("persistence store should exist");

        for timestamp_ms in [10, 20, 30] {
            let entry = crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: Uuid::new_v4().to_string(),
                to_node_id: Uuid::new_v4().to_string(),
                timestamp_ms,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
            };
            store
                .archive_append_traversal(&entry)
                .expect("timeline archive write should succeed");
        }

        for timestamp_ms in [40, 50, 60] {
            let entry = crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: Uuid::new_v4().to_string(),
                to_node_id: Uuid::new_v4().to_string(),
                timestamp_ms,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
            };
            store
                .archive_dissolved_traversal(&entry)
                .expect("dissolved archive write should succeed");
        }
    }

    app.apply_reducer_intents([GraphIntent::AutoCurateHistoryTimeline { keep_latest: 2 }]);
    assert_eq!(
        app.history_manager_archive_counts(),
        (2, 3),
        "timeline curation should not change dissolved archive count"
    );
    let timeline_timestamps: Vec<u64> = app
        .history_manager_timeline_entries(usize::MAX)
        .into_iter()
        .map(|entry| match entry {
            crate::services::persistence::types::LogEntry::AppendTraversal {
                timestamp_ms, ..
            } => timestamp_ms,
            other => panic!("expected traversal entry, got {other:?}"),
        })
        .collect();
    assert_eq!(timeline_timestamps, vec![30, 20]);

    app.apply_reducer_intents([GraphIntent::AutoCurateHistoryDissolved { keep_latest: 1 }]);
    assert_eq!(
        app.history_manager_archive_counts(),
        (2, 1),
        "dissolved curation should not change curated timeline count"
    );
    let dissolved_timestamps: Vec<u64> = app
        .history_manager_dissolved_entries(usize::MAX)
        .into_iter()
        .map(|entry| match entry {
            crate::services::persistence::types::LogEntry::AppendTraversal {
                timestamp_ms, ..
            } => timestamp_ms,
            other => panic!("expected traversal entry, got {other:?}"),
        })
        .collect();
    assert_eq!(dissolved_timestamps, vec![60]);
}

#[test]
fn history_timeline_index_entries_are_exposed_from_persistence() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    let from = Uuid::new_v4().to_string();
    let to = Uuid::new_v4().to_string();

    {
        let store = app
            .services
            .persistence
            .as_mut()
            .expect("persistence store should exist");
        store.log_mutation(
            &crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: from.clone(),
                to_node_id: to.clone(),
                timestamp_ms: 10,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
            },
        );
        store.log_mutation(
            &crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: from,
                to_node_id: to,
                timestamp_ms: 20,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Forward,
            },
        );
    }

    let idx = app.history_timeline_index_entries(usize::MAX);
    assert_eq!(idx.len(), 2);
    assert_eq!(idx[0].timestamp_ms, 20);
    assert_eq!(idx[1].timestamp_ms, 10);
    assert!(idx[0].log_position > idx[1].log_position);
}

#[test]
fn history_health_summary_tracks_preview_and_return_to_present_failure() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
    let preview = app.history_health_summary();
    assert!(preview.preview_mode_active);
    assert!(!preview.last_preview_isolation_violation);
    assert!(!preview.replay_in_progress);
    assert!(preview.replay_cursor.is_none());
    assert!(preview.replay_total_steps.is_none());
    assert!(preview.last_return_to_present_result.is_none());

    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayStarted]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayProgress {
        cursor: 2,
        total_steps: 5,
    }]);
    let replay = app.history_health_summary();
    assert!(replay.replay_in_progress);
    assert_eq!(replay.replay_cursor, Some(2));
    assert_eq!(replay.replay_total_steps, Some(5));

    app.apply_reducer_intents([GraphIntent::HistoryTimelinePreviewIsolationViolation {
        detail: "attempted live mutation".to_string(),
    }]);
    let violation = app.history_health_summary();
    assert!(violation.last_preview_isolation_violation);
    assert_eq!(
        violation.recent_failure_reason_bucket.as_deref(),
        Some("preview_isolation_violation")
    );

    app.apply_reducer_intents([GraphIntent::HistoryTimelineReturnToPresentFailed {
        detail: "cursor invalid".to_string(),
    }]);
    let result = app.history_health_summary();
    assert_eq!(
        result.last_return_to_present_result.as_deref(),
        Some("failed: cursor invalid")
    );
    assert_eq!(
        result.recent_failure_reason_bucket.as_deref(),
        Some("return_to_present_failed")
    );
}

#[test]
fn history_preview_blocks_graph_mutations_and_records_isolation_violation() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
    let before_count = app.workspace.domain.graph.node_count();

    app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    let after_count = app.workspace.domain.graph.node_count();
    assert_eq!(before_count, after_count);
    let health = app.history_health_summary();
    assert!(health.preview_mode_active);
    assert!(health.last_preview_isolation_violation);
    assert!(
        health
            .last_error
            .as_deref()
            .is_some_and(|msg| msg.contains("preview_isolation_violation"))
    );
}

#[test]
fn history_replay_advance_and_reset_follow_cursor_contract() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplaySetTotal { total_steps: 5 }]);

    let seeded = app.history_health_summary();
    assert_eq!(seeded.replay_cursor, Some(0));
    assert_eq!(seeded.replay_total_steps, Some(5));

    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 3 }]);
    let mid = app.history_health_summary();
    assert!(mid.replay_in_progress);
    assert_eq!(mid.replay_cursor, Some(3));

    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 10 }]);
    let done = app.history_health_summary();
    assert!(!done.replay_in_progress);
    assert_eq!(done.replay_cursor, Some(5));

    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayReset]);
    let reset = app.history_health_summary();
    assert!(!reset.replay_in_progress);
    assert_eq!(reset.replay_cursor, Some(0));
}

#[test]
fn history_preview_replay_builds_detached_graph_without_mutating_live_state() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let later = Uuid::new_v4();

    {
        let store = app
            .services
            .persistence
            .as_mut()
            .expect("persistence store should exist");
        store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
            node_id: from.to_string(),
            url: "https://from.example".to_string(),
            position_x: 0.0,
            position_y: 0.0,
            timestamp_ms: 0,
        });
        store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
            node_id: to.to_string(),
            url: "https://to.example".to_string(),
            position_x: 32.0,
            position_y: 0.0,
            timestamp_ms: 0,
        });
        store.log_mutation(
            &crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: from.to_string(),
                to_node_id: to.to_string(),
                timestamp_ms: 1_000,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Forward,
            },
        );
        store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
            node_id: later.to_string(),
            url: "https://later.example".to_string(),
            position_x: 64.0,
            position_y: 0.0,
            timestamp_ms: 2_000,
        });
        app.workspace.domain.graph = store.recover().expect("full graph recovery");
    }

    assert_eq!(app.workspace.domain.graph.node_count(), 3);

    app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplaySetTotal { total_steps: 1 }]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 1 }]);

    assert_eq!(app.workspace.domain.graph.node_count(), 3);
    let preview_graph = app
        .workspace
        .graph_runtime
        .history_preview_graph
        .as_ref()
        .expect("preview graph should be populated");
    assert_eq!(preview_graph.node_count(), 2);
    assert!(preview_graph.get_node_key_by_id(later).is_none());

    app.apply_reducer_intents([GraphIntent::ExitHistoryTimelinePreview]);
    assert_eq!(app.workspace.domain.graph.node_count(), 3);
    assert!(app.workspace.graph_runtime.history_preview_graph.is_none());
    assert_eq!(
        app.workspace
            .graph_runtime
            .history_last_return_to_present_result
            .as_deref(),
        Some("restored")
    );
}

#[test]
fn history_preview_replay_does_not_append_persistence_log_entries() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();

    {
        let store = app
            .services
            .persistence
            .as_mut()
            .expect("persistence store should exist");
        store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
            node_id: from.to_string(),
            url: "https://from.example".to_string(),
            position_x: 0.0,
            position_y: 0.0,
            timestamp_ms: 0,
        });
        store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
            node_id: to.to_string(),
            url: "https://to.example".to_string(),
            position_x: 32.0,
            position_y: 0.0,
            timestamp_ms: 0,
        });
        store.log_mutation(
            &crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id: from.to_string(),
                to_node_id: to.to_string(),
                timestamp_ms: 1_000,
                trigger: crate::services::persistence::types::PersistedNavigationTrigger::Forward,
            },
        );
    }

    let before_log_entries = app
        .services
        .persistence
        .as_ref()
        .expect("persistence store should exist")
        .log_entry_count_for_tests();

    app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplaySetTotal { total_steps: 1 }]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 1 }]);
    app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayReset]);
    app.apply_reducer_intents([GraphIntent::ExitHistoryTimelinePreview]);

    let after_log_entries = app
        .services
        .persistence
        .as_ref()
        .expect("persistence store should exist")
        .log_entry_count_for_tests();
    assert_eq!(before_log_entries, after_log_entries);
}

#[test]
fn test_intent_create_user_grouped_edge_adds_single_edge() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

    app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from,
        to,
        label: None,
    }]);

    let grouped =
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped);

    let count = app
        .workspace
        .domain
        .graph
        .inner
        .edge_references()
        .filter(|edge| {
            edge.source() == from && edge.target() == to && edge.weight().has_relation(grouped)
        })
        .count();
    assert_eq!(count, 1);
}

#[test]
fn test_intent_create_user_grouped_edge_is_idempotent() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

    app.apply_reducer_intents([
        GraphIntent::CreateUserGroupedEdge {
            from,
            to,
            label: None,
        },
        GraphIntent::CreateUserGroupedEdge {
            from,
            to,
            label: None,
        },
    ]);

    let grouped =
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped);

    let count = app
        .workspace
        .domain
        .graph
        .inner
        .edge_references()
        .filter(|edge| {
            edge.source() == from && edge.target() == to && edge.weight().has_relation(grouped)
        })
        .count();
    assert_eq!(count, 1);
}

#[test]
fn test_intent_create_user_grouped_edge_from_primary_selection_noop_for_single_select() {
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    app.select_node(a, false);

    app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdgeFromPrimarySelection]);

    let grouped =
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped);

    let count = app
        .workspace
        .domain
        .graph
        .inner
        .edge_references()
        .filter(|edge| edge.weight().has_relation(grouped))
        .count();
    assert_eq!(count, 0);
}

#[test]
fn test_execute_edge_command_connect_selected_pair() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

    app.select_node(from, false);
    app.select_node(to, true);
    app.workspace.graph_runtime.physics.base.is_running = false;

    app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
        command: EdgeCommand::ConnectSelectedPair,
    }]);

    assert!(app.has_relation(
        from,
        to,
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped),
    ));
    assert!(app.workspace.graph_runtime.physics.base.is_running);
}

#[test]
fn test_selection_ordered_pair_uses_first_selected_as_source() {
    let mut app = GraphBrowserApp::new_for_testing();
    let first = app
        .workspace
        .domain
        .graph
        .add_node("https://first.com".into(), Point2D::new(0.0, 0.0));
    let second = app
        .workspace
        .domain
        .graph
        .add_node("https://second.com".into(), Point2D::new(10.0, 0.0));

    app.select_node(first, false);
    app.select_node(second, true);

    assert_eq!(
        app.focused_selection().ordered_pair(),
        Some((first, second))
    );
}

#[test]
fn test_execute_edge_command_remove_user_edge_removes_both_directions() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

    app.add_user_grouped_edge_if_missing(from, to, None);
    app.add_user_grouped_edge_if_missing(to, from, None);
    app.select_node(from, false);
    app.select_node(to, true);
    app.workspace.graph_runtime.physics.base.is_running = false;

    app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
        command: EdgeCommand::RemoveUserEdge,
    }]);

    let grouped =
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped);
    assert!(!app.has_relation(from, to, grouped));
    assert!(!app.has_relation(to, from, grouped));
    assert!(app.workspace.graph_runtime.physics.base.is_running);
}

#[test]
fn test_execute_edge_command_pin_and_unpin_selected() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://pin.com".into(), Point2D::new(0.0, 0.0));
    app.select_node(key, false);

    app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
        command: EdgeCommand::PinSelected,
    }]);
    assert!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|node| node.is_pinned)
    );

    app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
        command: EdgeCommand::UnpinSelected,
    }]);
    assert!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|node| !node.is_pinned)
    );
}

#[test]
fn test_add_node_and_sync_reheats_physics() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.graph_runtime.physics.base.is_running = false;
    app.workspace.graph_runtime.drag_release_frames_remaining = 5;

    app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

    assert!(app.workspace.graph_runtime.physics.base.is_running);
    assert_eq!(app.workspace.graph_runtime.drag_release_frames_remaining, 0);
}

#[test]
fn test_reheat_physics_intent_enables_simulation() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.graph_runtime.physics.base.is_running = false;
    app.workspace.graph_runtime.drag_release_frames_remaining = 5;

    app.apply_reducer_intents([GraphIntent::ReheatPhysics]);

    assert!(app.workspace.graph_runtime.physics.base.is_running);
    assert_eq!(app.workspace.graph_runtime.drag_release_frames_remaining, 0);
}

#[test]
fn test_set_camera_fit_lock_clears_pending_drag_release_decay() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    app.workspace.graph_runtime.drag_release_frames_remaining = 7;

    app.set_camera_fit_locked(true);

    assert!(app.camera_fit_locked());
    assert_eq!(app.workspace.graph_runtime.drag_release_frames_remaining, 0);
}

#[test]
fn test_drag_release_keeps_physics_paused_when_camera_fit_lock_enabled() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);
    app.workspace.graph_runtime.physics.base.is_running = false;
    app.set_camera_fit_locked(true);

    app.set_interacting(true);
    app.set_interacting(false);

    assert!(!app.workspace.graph_runtime.physics.base.is_running);
    assert_eq!(app.workspace.graph_runtime.drag_release_frames_remaining, 0);
}

#[test]
fn test_toggle_primary_node_pin_toggles_selected_primary() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://pin.com".into(), Point2D::new(0.0, 0.0));
    app.select_node(key, false);

    app.apply_reducer_intents([GraphIntent::TogglePrimaryNodePin]);
    assert!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|node| node.is_pinned)
    );

    app.apply_reducer_intents([GraphIntent::TogglePrimaryNodePin]);
    assert!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|node| !node.is_pinned)
    );
}

#[test]
fn test_intent_remove_edge_removes_matching_type_only() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app.add_node_and_sync("https://from.com".into(), Point2D::new(0.0, 0.0));
    let to = app.add_node_and_sync("https://to.com".into(), Point2D::new(100.0, 0.0));

    let _ = app.assert_relation_and_sync(
        from,
        to,
        crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::Hyperlink,
            label: None,
            decay_progress: None,
        },
    );
    let _ = app.assert_relation_and_sync(
        from,
        to,
        crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::UserGrouped,
            label: None,
            decay_progress: None,
        },
    );

    app.apply_reducer_intents([GraphIntent::RemoveEdge {
        from,
        to,
        selector: crate::graph::RelationSelector::Semantic(
            crate::graph::SemanticSubKind::UserGrouped,
        ),
    }]);

    let has_user_grouped = app.has_relation(
        from,
        to,
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped),
    );
    let has_hyperlink = app.has_relation(
        from,
        to,
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::Hyperlink),
    );
    assert!(!has_user_grouped);
    assert!(has_hyperlink);
}

#[test]
fn test_remove_edges_and_log_reports_removed_count() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app.add_node_and_sync("https://from.com".into(), Point2D::new(0.0, 0.0));
    let to = app.add_node_and_sync("https://to.com".into(), Point2D::new(100.0, 0.0));

    let _ = app.assert_relation_and_sync(
        from,
        to,
        crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::UserGrouped,
            label: None,
            decay_progress: None,
        },
    );
    let _ = app.assert_relation_and_sync(
        from,
        to,
        crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::UserGrouped,
            label: None,
            decay_progress: None,
        },
    );

    let removed = app.retract_relations_and_log(
        from,
        to,
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped),
    );
    assert_eq!(removed, 1);
    assert!(!app.has_relation(
        from,
        to,
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped),
    ));
}

#[test]
fn test_delete_import_record_intent_removes_record_and_provenance() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.add_node_and_sync("https://imported.example".into(), Point2D::new(0.0, 0.0));
    let node_id = app
        .workspace
        .domain
        .graph
        .get_node(key)
        .expect("node")
        .id
        .to_string();
    assert!(
        app.workspace
            .domain
            .graph
            .set_import_records(vec![crate::graph::ImportRecord {
                record_id: "import-record:test".to_string(),
                source_id: "import:test".to_string(),
                source_label: "Test import".to_string(),
                imported_at_secs: 1_763_500_800,
                memberships: vec![crate::graph::ImportRecordMembership {
                    node_id,
                    suppressed: false,
                }],
            }])
    );

    app.apply_reducer_intents([GraphIntent::DeleteImportRecord {
        record_id: "import-record:test".to_string(),
    }]);

    assert!(app.workspace.domain.graph.import_records().is_empty());
    assert!(
        app.workspace
            .domain
            .graph
            .node_import_provenance(key)
            .expect("provenance slice")
            .is_empty()
    );
}

#[test]
fn test_suppress_import_record_membership_intent_hides_selected_node_from_imported_group() {
    let mut app = GraphBrowserApp::new_for_testing();
    let hidden = app.add_node_and_sync("https://hidden.example".into(), Point2D::new(0.0, 0.0));
    let visible = app.add_node_and_sync("https://visible.example".into(), Point2D::new(10.0, 0.0));
    let hidden_id = app
        .workspace
        .domain
        .graph
        .get_node(hidden)
        .expect("hidden")
        .id
        .to_string();
    let visible_id = app
        .workspace
        .domain
        .graph
        .get_node(visible)
        .expect("visible")
        .id
        .to_string();
    assert!(
        app.workspace
            .domain
            .graph
            .set_import_records(vec![crate::graph::ImportRecord {
                record_id: "import-record:test".to_string(),
                source_id: "import:test".to_string(),
                source_label: "Test import".to_string(),
                imported_at_secs: 1_763_500_800,
                memberships: vec![
                    crate::graph::ImportRecordMembership {
                        node_id: hidden_id,
                        suppressed: false,
                    },
                    crate::graph::ImportRecordMembership {
                        node_id: visible_id,
                        suppressed: false,
                    },
                ],
            }])
    );

    app.apply_reducer_intents([GraphIntent::SuppressImportRecordMembership {
        record_id: "import-record:test".to_string(),
        key: hidden,
    }]);

    assert!(
        app.workspace
            .domain
            .graph
            .node_import_provenance(hidden)
            .expect("hidden provenance slice")
            .is_empty()
    );
    assert_eq!(
        app.workspace
            .domain
            .graph
            .import_record_member_keys("import-record:test"),
        vec![visible]
    );
}

#[test]
fn test_promote_import_record_to_user_group_intent_creates_bidirectional_edges_from_anchor() {
    let mut app = GraphBrowserApp::new_for_testing();
    let anchor = app.add_node_and_sync("https://anchor.example".into(), Point2D::new(0.0, 0.0));
    let peer = app.add_node_and_sync("https://peer.example".into(), Point2D::new(10.0, 0.0));
    let other_peer =
        app.add_node_and_sync("https://other-peer.example".into(), Point2D::new(20.0, 0.0));
    let anchor_id = app
        .workspace
        .domain
        .graph
        .get_node(anchor)
        .expect("anchor")
        .id
        .to_string();
    let peer_id = app
        .workspace
        .domain
        .graph
        .get_node(peer)
        .expect("peer")
        .id
        .to_string();
    let other_peer_id = app
        .workspace
        .domain
        .graph
        .get_node(other_peer)
        .expect("other peer")
        .id
        .to_string();
    assert!(
        app.workspace
            .domain
            .graph
            .set_import_records(vec![crate::graph::ImportRecord {
                record_id: "import-record:test".to_string(),
                source_id: "import:test".to_string(),
                source_label: "Test import".to_string(),
                imported_at_secs: 1_763_500_800,
                memberships: vec![
                    crate::graph::ImportRecordMembership {
                        node_id: anchor_id,
                        suppressed: false,
                    },
                    crate::graph::ImportRecordMembership {
                        node_id: peer_id,
                        suppressed: false,
                    },
                    crate::graph::ImportRecordMembership {
                        node_id: other_peer_id,
                        suppressed: false,
                    },
                ],
            }])
    );

    app.apply_reducer_intents([GraphIntent::PromoteImportRecordToUserGroup {
        record_id: "import-record:test".to_string(),
        anchor,
    }]);

    let grouped =
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped);
    assert!(app.has_relation(anchor, peer, grouped));
    assert!(app.has_relation(peer, anchor, grouped));
    assert!(app.has_relation(anchor, other_peer, grouped));
    assert!(app.has_relation(other_peer, anchor, grouped));
    assert!(!app.has_relation(peer, other_peer, grouped));
    assert!(!app.has_relation(other_peer, peer, grouped));
}

#[test]
fn test_history_changed_is_authoritative_when_url_callback_stays_latest() {
    let mut app = GraphBrowserApp::new_for_testing();
    let step1 = app.add_node_and_sync(
        "https://site.example/?step=1".into(),
        Point2D::new(0.0, 0.0),
    );
    let step2 = app.add_node_and_sync(
        "https://site.example/?step=2".into(),
        Point2D::new(10.0, 0.0),
    );
    let wv = test_webview_id();
    app.map_webview_to_node(wv, step2);
    if let Some(node) = app.workspace.domain.graph.get_node_mut(step2) {
        node.history_entries = vec![
            "https://site.example/?step=0".into(),
            "https://site.example/?step=1".into(),
            "https://site.example/?step=2".into(),
        ];
        node.history_index = 2;
    }

    // Mirrors observed delegate behavior: URL callback can stay at the latest route
    // while history callback index moves backward.
    app.apply_reducer_intents([
        GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://site.example/?step=2".into(),
        },
        GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec![
                "https://site.example/?step=0".into(),
                "https://site.example/?step=1".into(),
                "https://site.example/?step=2".into(),
            ],
            current: 1,
        },
    ]);

    let node = app.workspace.domain.graph.get_node(step2).unwrap();
    assert_eq!(node.history_index, 1);

    let has_edge = app
        .workspace
        .domain
        .graph
        .edges()
        .any(|e| e.edge_type == EdgeType::History && e.from == step2 && e.to == step1);
    assert!(has_edge);
}

#[test]
fn test_intent_webview_title_changed_updates_and_ignores_empty() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://title.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    let original_title = app
        .workspace
        .domain
        .graph
        .get_node(key)
        .unwrap()
        .title
        .clone();

    app.apply_reducer_intents([GraphIntent::WebViewTitleChanged {
        webview_id: wv,
        title: Some("".into()),
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().title,
        original_title
    );

    app.apply_reducer_intents([GraphIntent::WebViewTitleChanged {
        webview_id: wv,
        title: Some("Hello".into()),
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().title,
        "Hello"
    );
}

#[test]
fn test_intent_thumbnail_and_favicon_update_node_metadata() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://assets.com".into(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([
        GraphIntent::SetNodeThumbnail {
            key,
            png_bytes: vec![1, 2, 3],
            width: 10,
            height: 20,
        },
        GraphIntent::SetNodeFavicon {
            key,
            rgba: vec![255, 0, 0, 255],
            width: 1,
            height: 1,
        },
    ]);

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.thumbnail_png.as_ref().unwrap().len(), 3);
    assert_eq!(node.thumbnail_width, 10);
    assert_eq!(node.thumbnail_height, 20);
    assert_eq!(node.favicon_rgba.as_ref().unwrap().len(), 4);
    assert_eq!(node.favicon_width, 1);
    assert_eq!(node.favicon_height, 1);
}

#[test]
fn test_conflict_delete_dominates_title_update_any_order() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://conflict-a.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.select_node(key, false);
    app.apply_reducer_intents([
        GraphIntent::RemoveSelectedNodes,
        GraphIntent::WebViewTitleChanged {
            webview_id: wv,
            title: Some("updated".into()),
        },
    ]);
    assert!(app.workspace.domain.graph.get_node(key).is_none());

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://conflict-b.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.select_node(key, false);
    app.apply_reducer_intents([
        GraphIntent::WebViewTitleChanged {
            webview_id: wv,
            title: Some("updated".into()),
        },
        GraphIntent::RemoveSelectedNodes,
    ]);
    assert!(app.workspace.domain.graph.get_node(key).is_none());
}

#[test]
fn test_conflict_delete_dominates_metadata_updates() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://conflict-meta.com".into(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.select_node(key, false);

    app.apply_reducer_intents([
        GraphIntent::RemoveSelectedNodes,
        GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://x.com".into()],
            current: 0,
        },
        GraphIntent::SetNodeThumbnail {
            key,
            png_bytes: vec![1, 2, 3],
            width: 8,
            height: 8,
        },
        GraphIntent::SetNodeFavicon {
            key,
            rgba: vec![0, 0, 0, 255],
            width: 1,
            height: 1,
        },
        GraphIntent::SetNodeUrl {
            key,
            new_url: "https://should-not-apply.com".into(),
        },
    ]);

    assert!(app.workspace.domain.graph.get_node(key).is_none());
}

#[test]
fn test_conflict_last_writer_wins_for_url_updates() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));
    app.apply_reducer_intents([
        GraphIntent::SetNodeUrl {
            key,
            new_url: "https://first.com".into(),
        },
        GraphIntent::SetNodeUrl {
            key,
            new_url: "https://second.com".into(),
        },
    ]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().url(),
        "https://second.com"
    );
}

#[test]
#[ignore]
fn perf_apply_intent_batch_10k_under_budget() {
    let mut app = GraphBrowserApp::new_for_testing();
    let mut intents = Vec::new();
    for i in 0..10_000 {
        intents.push(GraphIntent::CreateNodeAtUrl {
            url: format!("https://perf/{i}"),
            position: Point2D::new((i % 100) as f32, (i / 100) as f32),
        });
    }
    let start = std::time::Instant::now();
    app.apply_reducer_intents(intents);
    let elapsed = start.elapsed();
    assert_eq!(app.workspace.domain.graph.node_count(), 10_000);
    assert!(
        elapsed < std::time::Duration::from_secs(4),
        "intent batch exceeded budget: {elapsed:?}"
    );
}

#[test]
fn test_camera_defaults() {
    let cam = Camera::new();
    assert_eq!(cam.zoom_min, 0.1);
    assert_eq!(cam.zoom_max, 10.0);
    assert_eq!(cam.current_zoom, 0.8);
}

#[test]
fn test_camera_clamp_within_range() {
    let cam = Camera::new();
    assert_eq!(cam.clamp(1.0), 1.0);
    assert_eq!(cam.clamp(5.0), 5.0);
    assert_eq!(cam.clamp(0.5), 0.5);
}

#[test]
fn test_camera_clamp_below_min() {
    let cam = Camera::new();
    assert_eq!(cam.clamp(0.05), 0.1);
    assert_eq!(cam.clamp(0.0), 0.1);
    assert_eq!(cam.clamp(-1.0), 0.1);
}

#[test]
fn test_camera_clamp_above_max() {
    let cam = Camera::new();
    assert_eq!(cam.clamp(15.0), 10.0);
    assert_eq!(cam.clamp(100.0), 10.0);
}

#[test]
fn test_camera_clamp_at_boundaries() {
    let cam = Camera::new();
    assert_eq!(cam.clamp(0.1), 0.1);
    assert_eq!(cam.clamp(10.0), 10.0);
}

#[test]
fn test_create_multiple_placeholder_nodes_unique_urls() {
    let mut app = GraphBrowserApp::new_for_testing();

    let k1 = app.create_new_node_near_center();
    let k2 = app.create_new_node_near_center();
    let k3 = app.create_new_node_near_center();

    // All three nodes must have distinct URLs
    let url1 = app
        .workspace
        .domain
        .graph
        .get_node(k1)
        .unwrap()
        .url()
        .to_string();
    let url2 = app
        .workspace
        .domain
        .graph
        .get_node(k2)
        .unwrap()
        .url()
        .to_string();
    let url3 = app
        .workspace
        .domain
        .graph
        .get_node(k3)
        .unwrap()
        .url()
        .to_string();

    assert_ne!(url1, url2);
    assert_ne!(url2, url3);
    assert_ne!(url1, url3);

    // All URLs start with about:blank#
    assert!(url1.starts_with("about:blank#"));
    assert!(url2.starts_with("about:blank#"));
    assert!(url3.starts_with("about:blank#"));

    // url_to_node should have 3 distinct entries
    assert_eq!(app.workspace.domain.graph.node_count(), 3);
    assert!(app.workspace.domain.graph.get_node_by_url(&url1).is_some());
    assert!(app.workspace.domain.graph.get_node_by_url(&url2).is_some());
    assert!(app.workspace.domain.graph.get_node_by_url(&url3).is_some());
}

#[test]
fn test_placeholder_id_scan_on_recovery() {
    let mut graph = Graph::new();
    graph.add_node("about:blank#5".to_string(), Point2D::new(0.0, 0.0));
    graph.add_node("about:blank#2".to_string(), Point2D::new(100.0, 0.0));
    graph.add_node("https://example.com".to_string(), Point2D::new(200.0, 0.0));

    let next_id = GraphBrowserApp::scan_max_placeholder_id(&graph);
    // Max is 5, so next should be 6
    assert_eq!(next_id, 6);
}

#[test]
fn test_placeholder_id_scan_empty_graph() {
    let graph = Graph::new();
    assert_eq!(GraphBrowserApp::scan_max_placeholder_id(&graph), 0);
}

// --- TEST-1: remove_selected_nodes ---

#[test]
fn test_remove_selected_nodes_single() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let _k2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));

    app.select_node(k1, false);
    app.remove_selected_nodes();

    assert_eq!(app.workspace.domain.graph.node_count(), 1);
    assert!(app.workspace.domain.graph.get_node(k1).is_none());
    assert!(app.focused_selection().is_empty());
}

#[test]
fn test_remove_selected_nodes_multi() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let k2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));
    let k3 = app
        .workspace
        .domain
        .graph
        .add_node("c".to_string(), Point2D::new(200.0, 0.0));

    app.select_node(k1, false);
    app.select_node(k2, true);
    app.remove_selected_nodes();

    assert_eq!(app.workspace.domain.graph.node_count(), 1);
    assert!(app.workspace.domain.graph.get_node(k3).is_some());
    assert!(app.focused_selection().is_empty());
}

#[test]
fn test_remove_selected_nodes_empty_selection() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));

    // No selection â€” should be a no-op
    app.remove_selected_nodes();
    assert_eq!(app.workspace.domain.graph.node_count(), 1);
}

#[test]
fn test_remove_selected_nodes_clears_webview_mapping() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));

    // Simulate a webview mapping
    let fake_wv_id = test_webview_id();
    app.map_webview_to_node(fake_wv_id, k1);
    assert!(app.get_node_for_webview(fake_wv_id).is_some());

    app.select_node(k1, false);
    app.remove_selected_nodes();

    // Mapping should be cleaned up
    assert!(app.get_node_for_webview(fake_wv_id).is_none());
    assert!(app.get_webview_for_node(k1).is_none());
}

// --- TEST-1: clear_graph ---

#[test]
fn test_clear_graph_resets_everything() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let k2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));

    app.select_node(k1, false);
    app.select_node(k2, false);

    let fake_wv_id = test_webview_id();
    app.map_webview_to_node(fake_wv_id, k1);
    app.demote_node_to_warm(k1);
    assert_eq!(app.workspace.graph_runtime.warm_cache_lru, vec![k1]);

    app.clear_graph();

    assert_eq!(app.workspace.domain.graph.node_count(), 0);
    assert!(app.focused_selection().is_empty());
    assert!(app.get_node_for_webview(fake_wv_id).is_none());
    assert!(app.workspace.graph_runtime.warm_cache_lru.is_empty());
    assert!(
        !app.workspace
            .workbench_session
            .workspace_has_unsaved_changes
    );
    assert!(!app.should_prompt_unsaved_workspace_save());
}

#[test]
fn test_navigator_projection_state_defaults_are_graph_owned() {
    let app = GraphBrowserApp::new_for_testing();

    assert_eq!(
        app.navigator_projection_state().mode,
        NavigatorProjectionMode::Workbench
    );
    assert_eq!(
        app.navigator_projection_state().projection_seed_source,
        NavigatorProjectionSeedSource::GraphContainment
    );
    assert_eq!(
        app.navigator_projection_state().sort_mode,
        NavigatorSortMode::Manual
    );
    assert!(app.navigator_projection_state().row_targets.is_empty());
    assert!(app.navigator_projection_state().selected_rows.is_empty());
}

#[test]
fn test_file_tree_projection_rebuild_populates_node_rows_for_graph_source() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node_key = app.workspace.domain.graph.add_node(
        "https://example.com/tree-node".to_string(),
        Point2D::new(0.0, 0.0),
    );
    let node_id = app
        .workspace
        .domain
        .graph
        .get_node(node_key)
        .map(|node| node.id)
        .expect("node must exist");

    app.apply_reducer_intents([GraphIntent::RebuildNavigatorProjection]);

    assert_eq!(
        app.navigator_projection_state()
            .row_targets
            .get(&format!("node:{node_id}")),
        Some(&NavigatorProjectionTarget::Node(node_key))
    );
}

#[test]
fn test_file_tree_projection_rebuild_populates_saved_view_rows_for_saved_view_source() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Saved View"));

    app.apply_reducer_intents([
        GraphIntent::SetNavigatorProjectionSeedSource {
            source: NavigatorProjectionSeedSource::SavedViewCollections,
        },
        GraphIntent::RebuildNavigatorProjection,
    ]);

    assert_eq!(
        app.navigator_projection_state()
            .row_targets
            .get(&format!("view:{}", view_id.as_uuid())),
        Some(&NavigatorProjectionTarget::SavedView(view_id))
    );
}

#[test]
fn test_file_tree_projection_rebuild_prunes_stale_selection_and_expansion_rows() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.set_navigator_selected_rows(["row:stale".to_string()]);
    app.set_navigator_expanded_rows(["row:stale".to_string()]);

    app.apply_reducer_intents([GraphIntent::RebuildNavigatorProjection]);

    assert!(app.navigator_projection_state().selected_rows.is_empty());
    assert!(app.navigator_projection_state().expanded_rows.is_empty());
}

#[test]
fn test_file_tree_projection_rebuild_populates_containment_rows_from_file_urls() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace
        .domain
        .graph
        .add_node("file:///tmp/a.txt".to_string(), Point2D::new(0.0, 0.0));
    app.workspace.domain.graph.add_node(
        "https://example.com/not-imported".to_string(),
        Point2D::new(1.0, 0.0),
    );

    app.apply_reducer_intents([
        GraphIntent::SetNavigatorProjectionSeedSource {
            source: NavigatorProjectionSeedSource::ContainmentRelations,
        },
        GraphIntent::RebuildNavigatorProjection,
    ]);

    let keys: Vec<&String> = app
        .navigator_projection_state()
        .row_targets
        .keys()
        .collect();
    assert!(
        keys.iter()
            .any(|row| row.starts_with("folder:file:///tmp/#"))
    );
    assert!(
        keys.iter()
            .any(|row| row.starts_with("domain:example.com#"))
    );
}

#[test]
fn test_file_tree_projection_root_filter_limits_containment_rows() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace
        .domain
        .graph
        .add_node("file:///tmp/a/a.txt".to_string(), Point2D::new(0.0, 0.0));
    app.workspace
        .domain
        .graph
        .add_node("file:///tmp/b/b.log".to_string(), Point2D::new(1.0, 0.0));

    app.apply_reducer_intents([
        GraphIntent::SetNavigatorProjectionSeedSource {
            source: NavigatorProjectionSeedSource::ContainmentRelations,
        },
        GraphIntent::SetNavigatorRootFilter {
            root_filter: Some("/tmp/a/".to_string()),
        },
        GraphIntent::RebuildNavigatorProjection,
    ]);

    let keys: Vec<&String> = app
        .navigator_projection_state()
        .row_targets
        .keys()
        .collect();
    assert_eq!(keys.len(), 1);
    assert!(keys[0].contains("/tmp/a/"));
}

#[test]
fn test_file_tree_projection_intents_apply_in_workspace_reducer() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([
        GraphIntent::SetNavigatorProjectionSeedSource {
            source: NavigatorProjectionSeedSource::ContainmentRelations,
        },
        GraphIntent::SetNavigatorProjectionMode {
            mode: NavigatorProjectionMode::Containment,
        },
        GraphIntent::SetNavigatorSortMode {
            sort_mode: NavigatorSortMode::NameDescending,
        },
        GraphIntent::SetNavigatorRootFilter {
            root_filter: Some("root:tests".to_string()),
        },
        GraphIntent::SetNavigatorSelectedRows {
            rows: vec!["row:selected".to_string()],
        },
        GraphIntent::SetNavigatorExpandedRows {
            rows: vec!["row:expanded".to_string()],
        },
    ]);

    assert_eq!(
        app.navigator_projection_state().mode,
        NavigatorProjectionMode::Containment
    );
    assert_eq!(
        app.navigator_projection_state().projection_seed_source,
        NavigatorProjectionSeedSource::ContainmentRelations
    );
    assert_eq!(
        app.navigator_projection_state().sort_mode,
        NavigatorSortMode::NameDescending
    );
    assert_eq!(
        app.navigator_projection_state().root_filter.as_deref(),
        Some("root:tests")
    );
    assert!(
        app.navigator_projection_state()
            .selected_rows
            .contains("row:selected")
    );
    assert!(
        app.navigator_projection_state()
            .expanded_rows
            .contains("row:expanded")
    );
}

#[test]
fn test_clear_graph_resets_navigator_projection_state() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.set_navigator_projection_seed_source(NavigatorProjectionSeedSource::ContainmentRelations);
    app.set_navigator_projection_mode(NavigatorProjectionMode::AllNodes);
    app.set_navigator_sort_mode(NavigatorSortMode::NameAscending);
    app.set_navigator_root_filter(Some("root:collections".to_string()));
    app.upsert_navigator_row_target(
        "row:stale",
        NavigatorProjectionTarget::SavedView(GraphViewId::new()),
    );
    app.set_navigator_selected_rows(["row:stale".to_string()]);

    app.clear_graph();

    assert_eq!(
        app.navigator_projection_state().mode,
        NavigatorProjectionMode::Workbench
    );
    assert_eq!(
        app.navigator_projection_state().projection_seed_source,
        NavigatorProjectionSeedSource::GraphContainment
    );
    assert_eq!(
        app.navigator_projection_state().sort_mode,
        NavigatorSortMode::Manual
    );
    assert!(app.navigator_projection_state().root_filter.is_none());
    assert!(app.navigator_projection_state().row_targets.is_empty());
    assert!(app.navigator_projection_state().selected_rows.is_empty());
}

// --- TEST-1: create_new_node_near_center ---

#[test]
fn test_create_new_node_near_center_empty_graph() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.create_new_node_near_center();

    assert_eq!(app.workspace.domain.graph.node_count(), 1);
    assert!(app.focused_selection().contains(&key));

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert!(node.url().starts_with("about:blank#"));
}

#[test]
fn test_create_new_node_near_center_selects_node() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("existing".to_string(), Point2D::new(0.0, 0.0));
    app.select_node(k1, false);

    let k2 = app.create_new_node_near_center();

    // New node should be selected, old one deselected
    assert_eq!(app.focused_selection().len(), 1);
    assert!(app.focused_selection().contains(&k2));
}

// --- TEST-1: demote/promote lifecycle ---

#[test]
fn test_promote_and_demote_node_lifecycle() {
    use crate::graph::NodeLifecycle;
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));

    // Default lifecycle is Cold
    assert!(matches!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    ));

    app.promote_node_to_active(key);
    assert!(matches!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    ));

    app.demote_node_to_cold(key);
    assert!(matches!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    ));
}

#[test]
fn test_demote_clears_webview_mapping() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let fake_wv_id = test_webview_id();

    app.map_webview_to_node(fake_wv_id, key);
    assert!(app.get_webview_for_node(key).is_some());

    app.demote_node_to_cold(key);
    assert!(app.get_webview_for_node(key).is_none());
    assert!(app.get_node_for_webview(fake_wv_id).is_none());
}

#[test]
fn test_demote_to_warm_sets_desired_lifecycle_without_mapping() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    app.promote_node_to_active(key);

    app.demote_node_to_warm(key);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Warm
    );
    assert!(app.workspace.graph_runtime.warm_cache_lru.is_empty());

    let wv_id = test_webview_id();
    app.map_webview_to_node(wv_id, key);
    app.demote_node_to_warm(key);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Warm
    );
    assert_eq!(app.workspace.graph_runtime.warm_cache_lru, vec![key]);
}

#[test]
fn test_policy_promote_does_not_auto_reactivate_crashed_node() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id: wv,
        reason: "boom".to_string(),
        has_backtrace: false,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
    assert!(app.runtime_crash_state_for_node(key).is_some());

    app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::ActiveTileVisible,
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
    assert!(app.runtime_crash_state_for_node(key).is_some());
}

#[test]
fn test_policy_user_select_can_reactivate_and_clear_crash_state() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id: wv,
        reason: "boom".to_string(),
        has_backtrace: false,
    }]);

    app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::UserSelect,
    }]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
    assert!(app.runtime_crash_state_for_node(key).is_none());
}

#[test]
fn test_crash_path_requires_explicit_clear_before_auto_reactivate() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id: wv,
        reason: "boom".to_string(),
        has_backtrace: false,
    }]);

    app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::ActiveTileVisible,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
    assert!(app.runtime_crash_state_for_node(key).is_some());

    app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::UserSelect,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
    assert!(app.runtime_crash_state_for_node(key).is_none());
}

#[test]
fn test_policy_explicit_close_clears_crash_state() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, key);
    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id: wv,
        reason: "boom".to_string(),
        has_backtrace: false,
    }]);
    assert!(app.runtime_crash_state_for_node(key).is_some());

    app.apply_reducer_intents([GraphIntent::DemoteNodeToCold {
        key,
        cause: LifecycleCause::ExplicitClose,
    }]);

    assert!(app.runtime_crash_state_for_node(key).is_none());
}

#[test]
fn test_mark_runtime_blocked_and_expiry_unblocks_node() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let retry_at = Instant::now() + Duration::from_millis(5);
    app.apply_reducer_intents([GraphIntent::MarkRuntimeBlocked {
        key,
        reason: RuntimeBlockReason::CreateRetryExhausted,
        retry_at: Some(retry_at),
    }]);
    assert!(app.is_runtime_blocked(key, Instant::now()));
    assert!(!app.is_runtime_blocked(key, retry_at + Duration::from_millis(1)));
    assert!(app.runtime_block_state_for_node(key).is_none());
}

#[test]
fn test_promote_clears_runtime_block_state() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    app.apply_reducer_intents([
        GraphIntent::MarkRuntimeBlocked {
            key,
            reason: RuntimeBlockReason::CreateRetryExhausted,
            retry_at: Some(Instant::now() + Duration::from_secs(1)),
        },
        GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::Restore,
        },
    ]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
    assert!(app.runtime_block_state_for_node(key).is_none());
}

#[test]
fn test_promote_to_active_removes_warm_cache_membership() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let wv_id = test_webview_id();
    app.map_webview_to_node(wv_id, key);
    app.demote_node_to_warm(key);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Warm
    );
    assert_eq!(app.workspace.graph_runtime.warm_cache_lru, vec![key]);

    app.promote_node_to_active(key);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
    assert!(app.workspace.graph_runtime.warm_cache_lru.is_empty());
}

#[test]
fn test_cache_churn_during_lifecycle_transitions_preserves_lifecycle_contract() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.workspace.domain.graph.add_node(
        "https://cache-lifecycle.example".to_string(),
        Point2D::new(0.0, 0.0),
    );

    for idx in 0..32 {
        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_parsed_metadata(
                format!("lifecycle:meta:{idx}"),
                serde_json::json!({"i": idx}),
            );
        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_suggestions(format!("lifecycle:suggest:{idx}"), vec![format!("q{idx}")]);
    }
    let _ = app
        .workspace
        .graph_runtime
        .runtime_caches
        .get_parsed_metadata("lifecycle:meta:0");
    let _ = app
        .workspace
        .graph_runtime
        .runtime_caches
        .get_parsed_metadata("lifecycle:meta:missing");

    app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::Restore,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    );
    assert_eq!(app.lifecycle_counts(), (1, 0, 0, 0));

    for idx in 32..64 {
        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_thumbnail(key, vec![idx as u8; 4]);
    }

    app.apply_reducer_intents([GraphIntent::DemoteNodeToWarm {
        key,
        cause: LifecycleCause::WorkspaceRetention,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Warm
    );
    assert_eq!(app.lifecycle_counts(), (0, 1, 0, 0));

    app.apply_reducer_intents([GraphIntent::DemoteNodeToCold {
        key,
        cause: LifecycleCause::MemoryPressureCritical,
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    );
    assert_eq!(app.lifecycle_counts(), (0, 0, 1, 0));
    assert!(app.runtime_block_state_for_node(key).is_none());
}

#[test]
fn test_unmap_webview_removes_warm_cache_membership() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let wv_id = test_webview_id();
    app.map_webview_to_node(wv_id, key);
    app.demote_node_to_warm(key);
    assert_eq!(app.workspace.graph_runtime.warm_cache_lru, vec![key]);

    let _ = app.unmap_webview(wv_id);
    assert!(app.workspace.graph_runtime.warm_cache_lru.is_empty());
}

#[test]
fn test_take_warm_cache_evictions_respects_lru_and_limit() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(1.0, 0.0));
    let key_c = app
        .workspace
        .domain
        .graph
        .add_node("c".to_string(), Point2D::new(2.0, 0.0));

    app.map_webview_to_node(test_webview_id(), key_a);
    app.demote_node_to_warm(key_a);
    app.map_webview_to_node(test_webview_id(), key_b);
    app.demote_node_to_warm(key_b);
    app.map_webview_to_node(test_webview_id(), key_c);
    app.demote_node_to_warm(key_c);

    assert_eq!(
        app.workspace.graph_runtime.warm_cache_lru,
        vec![key_a, key_b, key_c]
    );

    app.workspace.graph_runtime.warm_cache_limit = 2;
    let evicted = app.take_warm_cache_evictions();
    assert_eq!(evicted, vec![key_a]);
    assert_eq!(
        app.workspace.graph_runtime.warm_cache_lru,
        vec![key_b, key_c]
    );
}

#[test]
fn test_take_active_webview_evictions_respects_limit_and_protection() {
    use std::collections::HashSet;

    let mut app = GraphBrowserApp::new_for_testing();
    let key_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(1.0, 0.0));
    let key_c = app
        .workspace
        .domain
        .graph
        .add_node("c".to_string(), Point2D::new(2.0, 0.0));
    let key_d = app
        .workspace
        .domain
        .graph
        .add_node("d".to_string(), Point2D::new(3.0, 0.0));

    for key in [key_a, key_b, key_c, key_d] {
        app.promote_node_to_active(key);
        app.map_webview_to_node(test_webview_id(), key);
    }

    app.workspace.graph_runtime.active_webview_limit = 3;
    let protected = HashSet::from([key_a]);
    let evicted = app.take_active_webview_evictions(&protected);

    assert_eq!(evicted.len(), 1);
    assert!(!protected.contains(&evicted[0]));
}

#[test]
fn test_take_active_webview_evictions_with_lower_limit() {
    use std::collections::HashSet;

    let mut app = GraphBrowserApp::new_for_testing();
    let key_a = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let key_b = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(1.0, 0.0));
    let key_c = app
        .workspace
        .domain
        .graph
        .add_node("c".to_string(), Point2D::new(2.0, 0.0));

    for key in [key_a, key_b, key_c] {
        app.promote_node_to_active(key);
        app.map_webview_to_node(test_webview_id(), key);
    }

    let evicted = app.take_active_webview_evictions_with_limit(1, &HashSet::new());
    assert_eq!(evicted.len(), 2);
}

#[test]
fn test_webview_crashed_demotes_node_and_unmaps_webview() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let wv_id = test_webview_id();

    app.promote_node_to_active(key);
    app.map_webview_to_node(wv_id, key);
    assert!(matches!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    ));

    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id: wv_id,
        reason: "gpu reset".to_string(),
        has_backtrace: false,
    }]);

    assert!(matches!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold
    ));
    assert_eq!(
        app.runtime_crash_state_for_node(key)
            .and_then(|state| state.message.as_deref()),
        Some("gpu reset")
    );
    assert!(app.get_node_for_webview(wv_id).is_none());
    assert!(app.get_webview_for_node(key).is_none());

    app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
        key,
        cause: LifecycleCause::Restore,
    }]);
    assert!(matches!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Active
    ));
    assert!(app.runtime_crash_state_for_node(key).is_none());
}

#[test]
fn test_clear_graph_clears_runtime_crash_state() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let wv_id = test_webview_id();
    app.map_webview_to_node(wv_id, key);
    app.apply_reducer_intents([GraphIntent::WebViewCrashed {
        webview_id: wv_id,
        reason: "boom".to_string(),
        has_backtrace: true,
    }]);
    assert!(app.runtime_crash_state_for_node(key).is_some());

    app.clear_graph();
    assert!(app.runtime_crash_state_for_node(key).is_none());
}

// --- TEST-1: webview mapping ---

#[test]
fn test_webview_mapping_bidirectional() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let wv_id = test_webview_id();

    app.map_webview_to_node(wv_id, key);

    assert_eq!(app.get_node_for_webview(wv_id), Some(key));
    assert_eq!(app.get_webview_for_node(key), Some(wv_id));
}

#[test]
fn test_unmap_webview() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let wv_id = test_webview_id();

    app.map_webview_to_node(wv_id, key);
    let unmapped_key = app.unmap_webview(wv_id);

    assert_eq!(unmapped_key, Some(key));
    assert!(app.get_node_for_webview(wv_id).is_none());
    assert!(app.get_webview_for_node(key).is_none());
}

#[test]
fn test_unmap_nonexistent_webview() {
    let mut app = GraphBrowserApp::new_for_testing();
    let wv_id = test_webview_id();

    assert_eq!(app.unmap_webview(wv_id), None);
}

#[test]
fn test_webview_node_mappings_iterator() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let k2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));
    let wv1 = test_webview_id();
    let wv2 = test_webview_id();

    app.map_webview_to_node(wv1, k1);
    app.map_webview_to_node(wv2, k2);

    let mappings: Vec<_> = app.webview_node_mappings().collect();
    assert_eq!(mappings.len(), 2);
}

// --- TEST-1: get_single_selected_node ---

#[test]
fn test_get_single_selected_node_one() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    app.select_node(key, false);

    assert_eq!(app.get_single_selected_node(), Some(key));
}

#[test]
fn test_get_single_selected_node_none() {
    let app = GraphBrowserApp::new_for_testing();
    assert_eq!(app.get_single_selected_node(), None);
}

#[test]
fn test_get_single_selected_node_multi() {
    let mut app = GraphBrowserApp::new_for_testing();
    let k1 = app
        .workspace
        .domain
        .graph
        .add_node("a".to_string(), Point2D::new(0.0, 0.0));
    let k2 = app
        .workspace
        .domain
        .graph
        .add_node("b".to_string(), Point2D::new(100.0, 0.0));
    app.select_node(k1, false);
    app.select_node(k2, true);

    assert_eq!(app.get_single_selected_node(), None);
}

// --- TEST-1: update_node_url_and_log ---

#[test]
fn test_update_node_url_and_log() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("old-url".to_string(), Point2D::new(0.0, 0.0));

    let old = app.update_node_url_and_log(key, "new-url".to_string());

    assert_eq!(old, Some("old-url".to_string()));
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().url(),
        "new-url"
    );
    // url_to_node should be updated
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("new-url")
            .is_some()
    );
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("old-url")
            .is_none()
    );
}

#[test]
fn test_update_node_url_nonexistent() {
    let mut app = GraphBrowserApp::new_for_testing();
    let fake_key = NodeKey::new(999);

    assert_eq!(app.update_node_url_and_log(fake_key, "x".to_string()), None);
}

#[test]
fn test_new_from_dir_recovers_logged_graph() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    {
        let mut store = GraphStore::open(path.clone()).unwrap();
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        store.log_mutation(&LogEntry::AddNode {
            node_id: id_a.to_string(),
            url: "https://a.com".to_string(),
            position_x: 10.0,
            position_y: 20.0,
            timestamp_ms: 0,
        });
        store.log_mutation(&LogEntry::AddNode {
            node_id: id_b.to_string(),
            url: "https://b.com".to_string(),
            position_x: 30.0,
            position_y: 40.0,
            timestamp_ms: 0,
        });
        store.log_mutation(&LogEntry::AddEdge {
            from_node_id: id_a.to_string(),
            to_node_id: id_b.to_string(),
            assertion: crate::services::persistence::types::PersistedEdgeAssertion::Semantic {
                sub_kind: crate::services::persistence::types::PersistedSemanticSubKind::Hyperlink,
                label: None,
                agent_decay_progress: None,
            },
        });
    }

    let app = GraphBrowserApp::new_from_dir(path);
    assert!(app.has_recovered_graph());
    assert_eq!(app.workspace.domain.graph.node_count(), 2);
    assert_eq!(app.workspace.domain.graph.edge_count(), 1);
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("https://a.com")
            .is_some()
    );
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("https://b.com")
            .is_some()
    );
}

#[test]
fn test_new_from_dir_scans_placeholder_ids_from_recovery() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    {
        let mut store = GraphStore::open(path.clone()).unwrap();
        let id = Uuid::new_v4();
        store.log_mutation(&LogEntry::AddNode {
            node_id: id.to_string(),
            url: "about:blank#5".to_string(),
            position_x: 0.0,
            position_y: 0.0,
            timestamp_ms: 0,
        });
    }

    let mut app = GraphBrowserApp::new_from_dir(path);
    let key = app.create_new_node_near_center();
    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.url(), "about:blank#6");
}

#[test]
fn test_clear_graph_and_persistence_in_memory_reset() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    app.select_node(key, false);

    app.clear_graph_and_persistence();

    assert_eq!(app.workspace.domain.graph.node_count(), 0);
    assert!(app.focused_selection().is_empty());
}

#[test]
fn test_clear_graph_and_persistence_wipes_store() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    {
        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.add_node_and_sync("https://persisted.com".to_string(), Point2D::new(1.0, 2.0));
        app.take_snapshot();
        app.clear_graph_and_persistence();
    }

    let recovered = GraphBrowserApp::new_from_dir(path);
    assert!(!recovered.has_recovered_graph());
    assert_eq!(recovered.workspace.domain.graph.node_count(), 0);
}

#[test]
fn test_resolve_frame_open_deterministic_fallback_without_recency_match() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let node_id = app.workspace.domain.graph.get_node(key).unwrap().id;

    let mut index = HashMap::new();
    index.insert(
        node_id,
        BTreeSet::from([
            "workspace-z".to_string(),
            "workspace-a".to_string(),
            "workspace-m".to_string(),
        ]),
    );
    app.init_membership_index(index);
    app.workspace
        .workbench_session
        .node_last_active_workspace
        .insert(node_id, (99, "workspace-missing".to_string()));

    for _ in 0..5 {
        assert_eq!(
            app.resolve_workspace_open(key, None),
            FrameOpenAction::RestoreFrame {
                name: "workspace-a".to_string(),
                node: key
            }
        );
    }
}

#[test]
fn test_resolve_frame_open_reason_honors_preferred_frame() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let node_id = app.workspace.domain.graph.get_node(key).unwrap().id;
    app.init_membership_index(HashMap::from([(
        node_id,
        BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
    )]));

    let (action, reason) = app.resolve_workspace_open_with_reason(key, Some("beta"));
    assert_eq!(
        action,
        FrameOpenAction::RestoreFrame {
            name: "beta".to_string(),
            node: key
        }
    );
    assert_eq!(reason, FrameOpenReason::PreferredFrame);
}

#[test]
fn test_resolve_frame_open_reason_recent_membership() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let node_id = app.workspace.domain.graph.get_node(key).unwrap().id;
    app.init_membership_index(HashMap::from([(
        node_id,
        BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
    )]));
    app.note_workspace_activated("beta", [key]);

    let (_, reason) = app.resolve_workspace_open_with_reason(key, None);
    assert_eq!(reason, FrameOpenReason::RecentMembership);
}

#[test]
fn test_resolve_frame_open_reason_no_membership() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let (_, reason) = app.resolve_workspace_open_with_reason(key, None);
    assert_eq!(reason, FrameOpenReason::NoMembership);
}

#[test]
fn test_new_from_dir_loads_persisted_toast_anchor_preference() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    {
        let mut store = GraphStore::open(path.clone()).unwrap();
        store
            .save_workspace_layout_json(GraphBrowserApp::SETTINGS_TOAST_ANCHOR_NAME, "top-left")
            .unwrap();
    }

    let app = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        app.workspace.chrome_ui.toast_anchor_preference,
        ToastAnchorPreference::TopLeft
    );
}

#[test]
fn test_keyboard_pan_step_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_keyboard_pan_step(27.0);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert!((reopened.keyboard_pan_step() - 27.0).abs() < 0.001);
}

#[test]
fn test_keyboard_pan_input_mode_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_keyboard_pan_input_mode(KeyboardPanInputMode::ArrowsOnly);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        reopened.keyboard_pan_input_mode(),
        KeyboardPanInputMode::ArrowsOnly
    );
}

#[test]
fn test_camera_pan_inertia_settings_persist_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_camera_pan_inertia_enabled(false);
    app.set_camera_pan_inertia_damping(0.92);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert!(!reopened.camera_pan_inertia_enabled());
    assert!((reopened.camera_pan_inertia_damping() - 0.92).abs() < 0.001);
}

#[test]
fn test_camera_starts_manual_without_pending_fit_command() {
    let app = GraphBrowserApp::new_for_testing();
    assert!(app.pending_camera_command().is_none());
}

#[test]
fn test_set_omnibar_settings_persist_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_omnibar_preferred_scope(OmnibarPreferredScope::ProviderDefault);
    app.set_omnibar_non_at_order(OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        reopened.workspace.chrome_ui.omnibar_preferred_scope,
        OmnibarPreferredScope::ProviderDefault
    );
    assert_eq!(
        reopened.workspace.chrome_ui.omnibar_non_at_order,
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal
    );
}

#[test]
fn test_wry_enabled_defaults_to_false() {
    let app = GraphBrowserApp::new_for_testing();
    assert!(!app.wry_enabled());
}

#[test]
fn test_wry_enabled_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_wry_enabled(true);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert!(reopened.wry_enabled());
}

#[test]
fn test_set_snapshot_interval_secs_updates_store() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    let mut app = GraphBrowserApp::new_from_dir(path);

    app.set_snapshot_interval_secs(45).unwrap();
    assert_eq!(app.snapshot_interval_secs(), Some(45));
}

#[test]
fn test_set_snapshot_interval_secs_without_persistence_fails() {
    let mut app = GraphBrowserApp::new_for_testing();
    assert!(app.set_snapshot_interval_secs(45).is_err());
    assert_eq!(app.snapshot_interval_secs(), None);
}

#[test]
fn test_nostr_subscriptions_persist_across_restart() {
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    let _guard = LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("nostr persistence test lock poisoned");

    crate::shell::desktop::runtime::registries::phase3_restore_nostr_subscriptions(&[])
        .expect("nostr subscriptions should clear before test");

    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    {
        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        crate::shell::desktop::runtime::registries::phase3_nostr_relay_subscribe_for_caller(
            "workspace:test",
            Some("timeline"),
            crate::shell::desktop::runtime::registries::nostr_core::NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.damus.io".to_string()],
            },
        )
        .expect("subscription should succeed");
        app.apply_reducer_intents([GraphIntent::PersistNostrSubscriptions]);
    }

    crate::shell::desktop::runtime::registries::phase3_restore_nostr_subscriptions(&[])
        .expect("nostr subscriptions should clear before reopen");

    let _reopened = GraphBrowserApp::new_from_dir(path);
    let persisted =
        crate::shell::desktop::runtime::registries::phase3_nostr_persisted_subscriptions();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].caller_id, "workspace:test");
    assert_eq!(persisted[0].requested_id.as_deref(), Some("timeline"));
    assert_eq!(persisted[0].filters.kinds, vec![1]);
}

#[test]
fn test_nostr_signer_settings_persist_across_restart_without_secret() {
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    let _guard = LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("nostr signer persistence test lock poisoned");

    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    let signer_secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let signer_keypair =
        secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &signer_secret);
    let (signer_pubkey, _) = secp256k1::XOnlyPublicKey::from_keypair(&signer_keypair);

    {
        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        crate::shell::desktop::runtime::registries::phase3_nostr_use_nip46_bunker_uri(&format!(
            "bunker://{}?relay=wss://relay.one&secret=shared-secret&perms=sign_event",
            signer_pubkey
        ))
        .expect("bunker uri should parse");
        app.save_persisted_nostr_signer_settings();
    }

    let _reopened = GraphBrowserApp::new_from_dir(path);
    match crate::shell::desktop::runtime::registries::phase3_nostr_signer_backend_snapshot() {
        crate::shell::desktop::runtime::registries::NostrSignerBackendSnapshot::Nip46Delegated {
            relay_urls,
            signer_pubkey: restored_pubkey,
            has_ephemeral_secret,
            requested_permissions,
            ..
        } => {
            assert_eq!(relay_urls, vec!["wss://relay.one".to_string()]);
            assert_eq!(restored_pubkey, signer_pubkey.to_string());
            assert!(!has_ephemeral_secret);
            assert_eq!(requested_permissions, vec!["sign_event".to_string()]);
        }
        other => panic!("expected delegated signer snapshot, got {other:?}"),
    }
}

#[test]
fn test_nostr_nip07_permissions_persist_across_restart() {
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    let _guard = LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("nostr nip07 persistence test lock poisoned");

    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    {
        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        crate::shell::desktop::runtime::registries::phase3_nostr_set_nip07_permission(
            "https://example.com/path",
            "signEvent",
            crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Allow,
        )
        .expect("nip07 permission should be accepted");
        app.save_persisted_nostr_nip07_permissions();
    }

    let _reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        crate::shell::desktop::runtime::registries::phase3_nostr_nip07_permission_grants(),
        vec![
            crate::shell::desktop::runtime::registries::Nip07PermissionGrant {
                origin: "https://example.com".to_string(),
                method: "signEvent".to_string(),
                decision:
                    crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Allow,
            }
        ]
    );
}

#[test]
fn test_registry_component_defaults_persist_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_default_registry_lens_id(Some("lens:default"));
    app.set_default_registry_physics_id(Some("physics:gas"));
    app.set_default_registry_theme_id(Some("theme:dark"));
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(reopened.default_registry_lens_id(), Some("lens:default"));
    assert_eq!(reopened.default_registry_physics_id(), Some("physics:gas"));
    assert_eq!(reopened.default_registry_theme_id(), Some("theme:dark"));
}

#[test]
fn default_registry_lens_setting_publishes_lens_invalidation_signal() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let observed = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&observed);
    let observer_id = crate::shell::desktop::runtime::registries::phase3_subscribe_signal(
        crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
        move |signal| {
            if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::LensChanged {
                    new_lens_id,
                },
            ) = &signal.kind
                && new_lens_id == crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT
            {
                seen.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        },
    );

    let mut app = GraphBrowserApp::new_for_testing();
    app.set_default_registry_lens_id(None);

    assert_eq!(observed.load(Ordering::Relaxed), 1);
    assert!(
        crate::shell::desktop::runtime::registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        )
    );
}

#[test]
fn test_workbench_host_pin_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_workbench_host_pinned(true);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert!(reopened.workbench_host_pinned());
}

#[test]
fn test_set_physics_profile_intent_updates_runtime_and_reheats() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.graph_runtime.physics.base.is_running = false;

    let view_id = GraphViewId::new();
    app.workspace.graph_runtime.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "Physics Test"),
    );

    app.apply_reducer_intents([GraphIntent::SetPhysicsProfile {
        profile_id: crate::registries::atomic::lens::PHYSICS_ID_GAS.to_string(),
    }]);

    assert_eq!(
        app.default_registry_physics_id(),
        Some(crate::registries::atomic::lens::PHYSICS_ID_GAS)
    );
    assert!(app.workspace.graph_runtime.physics.base.is_running);
    assert_eq!(
        crate::shell::desktop::runtime::registries::phase3_resolve_active_physics_profile()
            .resolved_id,
        crate::registries::atomic::lens::PHYSICS_ID_GAS
    );
    assert_eq!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .resolved_physics_profile()
            .name,
        "Gas"
    );
}

#[test]
fn test_set_theme_intent_updates_runtime_and_workspace_setting() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::SetTheme {
        theme_id: crate::shell::desktop::runtime::registries::theme::THEME_ID_DARK.to_string(),
    }]);

    assert_eq!(
        app.default_registry_theme_id(),
        Some(crate::shell::desktop::runtime::registries::theme::THEME_ID_DARK)
    );
    assert_eq!(
        crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(None).resolved_id,
        crate::shell::desktop::runtime::registries::theme::THEME_ID_DARK
    );
}

#[test]
fn suggest_node_tags_intent_stores_display_only_suggestions_and_prunes_on_tag_commit() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.add_node_and_sync(
        "https://math.example.edu".to_string(),
        Point2D::new(0.0, 0.0),
    );

    app.apply_reducer_intents([GraphIntent::SuggestNodeTags {
        key,
        suggestions: vec![
            "udc:51".to_string(),
            "udc:519.6".to_string(),
            "unknown".to_string(),
        ],
    }]);

    assert_eq!(
        app.workspace
            .graph_runtime
            .suggested_semantic_tags
            .get(&key),
        Some(&vec!["udc:51".to_string(), "udc:519.6".to_string()])
    );
    assert!(app.canonical_tags_for_node(key).is_empty());

    app.apply_reducer_intents([GraphIntent::TagNode {
        key,
        tag: "519.6".to_string(),
    }]);

    assert_eq!(
        app.workspace
            .graph_runtime
            .suggested_semantic_tags
            .get(&key),
        Some(&vec!["udc:51".to_string()])
    );
    assert!(app.node_has_canonical_tag(key, "udc:519.6"));
}

// ---------------------------------------------------------------------------
// Faceted filter — spec §9 acceptance criteria
// "Reducer owns filter truth": UI submits intent; reducer result drives visible
// projection. "Filtering does not mutate graph truth": node/edge identity and
// lifecycle unchanged across apply/clear.
// Source: faceted_filter_surface_spec.md §9
// ---------------------------------------------------------------------------

#[test]
fn test_set_view_filter_intent_applies_filter_to_view() {
    let mut app = GraphBrowserApp::new_for_testing();

    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Filter"));

    let a = app.add_node_and_sync("https://a.test/".into(), Point2D::new(0.0, 0.0));
    let b = app.add_node_and_sync("https://b.test/".into(), Point2D::new(10.0, 0.0));

    // Tag only node `a`
    app.apply_reducer_intents([GraphIntent::TagNode {
        key: a,
        tag: "starred".to_string(),
    }]);

    let expr = crate::model::graph::filter::parse_omnibar_facet_token("facet:udc_classes=starred")
        .unwrap();

    app.apply_reducer_intents([GraphIntent::SetViewFilter {
        view_id,
        expr: Some(expr.clone()),
    }]);

    // active_filter is set on the view
    assert!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .active_filter
            .is_some()
    );

    // evaluate against domain graph: `a` matches, `b` does not
    let summary = crate::model::graph::filter::evaluate_filter_result(app.domain_graph(), &expr);
    let matched: std::collections::HashSet<_> =
        summary.result.matched_nodes.iter().copied().collect();
    assert!(matched.contains(&a), "tagged node must match the filter");
    assert!(
        !matched.contains(&b),
        "untagged node must not match the filter"
    );
}

#[test]
fn test_clear_view_filter_intent_removes_active_filter() {
    let mut app = GraphBrowserApp::new_for_testing();

    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Filter"));

    let _a = app.add_node_and_sync("https://a.test/".into(), Point2D::new(0.0, 0.0));

    let expr = crate::model::graph::filter::parse_omnibar_facet_token("facet:lifecycle").unwrap();

    app.apply_reducer_intents([GraphIntent::SetViewFilter {
        view_id,
        expr: Some(expr),
    }]);
    assert!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .active_filter
            .is_some()
    );

    app.apply_reducer_intents([GraphIntent::ClearViewFilter { view_id }]);

    assert!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .active_filter
            .is_none(),
        "ClearViewFilter must remove the active filter from the view"
    );
}

#[test]
fn test_set_view_filter_does_not_mutate_graph_node_identity() {
    let mut app = GraphBrowserApp::new_for_testing();

    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Filter"));

    let node = app.add_node_and_sync("https://identity.test/".into(), Point2D::new(0.0, 0.0));
    let url_before = app
        .workspace
        .domain
        .graph
        .get_node(node)
        .unwrap()
        .url()
        .to_string();
    let lifecycle_before = app.workspace.domain.graph.get_node(node).unwrap().lifecycle;

    let expr =
        crate::model::graph::filter::parse_omnibar_facet_token("facet:lifecycle=Active").unwrap();

    app.apply_reducer_intents([GraphIntent::SetViewFilter {
        view_id,
        expr: Some(expr),
    }]);
    app.apply_reducer_intents([GraphIntent::ClearViewFilter { view_id }]);

    let url_after = app
        .workspace
        .domain
        .graph
        .get_node(node)
        .unwrap()
        .url()
        .to_string();
    let lifecycle_after = app.workspace.domain.graph.get_node(node).unwrap().lifecycle;

    assert_eq!(
        url_before, url_after,
        "filter apply/clear must not mutate node URL"
    );
    assert_eq!(
        lifecycle_before, lifecycle_after,
        "filter apply/clear must not mutate node lifecycle"
    );
    assert_eq!(
        app.workspace.domain.graph.node_count(),
        1,
        "filter apply/clear must not add or remove nodes"
    );
}

// ---------------------------------------------------------------------------
// Node classification intents — Stage A enrichment spec
// "tags/classifications survive restart and replay; reducer-only mutation
// boundary is enforced"
// Source: graph_enrichment_plan.md §Stage A
// ---------------------------------------------------------------------------

#[test]
fn test_assign_classification_intent_adds_record() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node = app.add_node_and_sync("https://example.com/".into(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::AssignClassification {
        key: node,
        classification: crate::model::graph::NodeClassification {
            scheme: crate::model::graph::ClassificationScheme::Udc,
            value: "udc:519.6".to_string(),
            label: Some("Computational mathematics".to_string()),
            confidence: 1.0,
            provenance: crate::model::graph::ClassificationProvenance::UserAuthored,
            status: crate::model::graph::ClassificationStatus::Accepted,
            primary: true,
        },
    }]);

    let classifications = app
        .workspace
        .domain
        .graph
        .node_classifications(node)
        .unwrap();
    assert_eq!(classifications.len(), 1);
    assert_eq!(classifications[0].value, "udc:519.6");
    assert_eq!(
        classifications[0].status,
        crate::model::graph::ClassificationStatus::Accepted
    );
}

#[test]
fn test_accept_reject_classification_intent_updates_status() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node = app.add_node_and_sync("https://example.com/".into(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::AssignClassification {
        key: node,
        classification: crate::model::graph::NodeClassification {
            scheme: crate::model::graph::ClassificationScheme::Udc,
            value: "udc:51".to_string(),
            label: None,
            confidence: 0.8,
            provenance: crate::model::graph::ClassificationProvenance::AgentSuggested,
            status: crate::model::graph::ClassificationStatus::Suggested,
            primary: false,
        },
    }]);

    // Accept it
    app.apply_reducer_intents([GraphIntent::AcceptClassification {
        key: node,
        scheme: crate::model::graph::ClassificationScheme::Udc,
        value: "udc:51".to_string(),
    }]);
    assert_eq!(
        app.workspace
            .domain
            .graph
            .node_classifications(node)
            .unwrap()[0]
            .status,
        crate::model::graph::ClassificationStatus::Accepted
    );

    // Reject it
    app.apply_reducer_intents([GraphIntent::RejectClassification {
        key: node,
        scheme: crate::model::graph::ClassificationScheme::Udc,
        value: "udc:51".to_string(),
    }]);
    assert_eq!(
        app.workspace
            .domain
            .graph
            .node_classifications(node)
            .unwrap()[0]
            .status,
        crate::model::graph::ClassificationStatus::Rejected
    );
}

#[test]
fn test_unassign_classification_intent_removes_record() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node = app.add_node_and_sync("https://example.com/".into(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::AssignClassification {
        key: node,
        classification: crate::model::graph::NodeClassification {
            scheme: crate::model::graph::ClassificationScheme::Udc,
            value: "udc:51".to_string(),
            label: None,
            confidence: 1.0,
            provenance: crate::model::graph::ClassificationProvenance::UserAuthored,
            status: crate::model::graph::ClassificationStatus::Accepted,
            primary: false,
        },
    }]);
    assert_eq!(
        app.workspace
            .domain
            .graph
            .node_classifications(node)
            .unwrap()
            .len(),
        1
    );

    app.apply_reducer_intents([GraphIntent::UnassignClassification {
        key: node,
        scheme: crate::model::graph::ClassificationScheme::Udc,
        value: "udc:51".to_string(),
    }]);
    assert!(
        app.workspace
            .domain
            .graph
            .node_classifications(node)
            .unwrap()
            .is_empty(),
        "UnassignClassification must remove the matching record"
    );
}

#[test]
fn test_classification_survives_snapshot_roundtrip() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    let node = app.add_node_and_sync("https://persist.test/".into(), Point2D::new(0.0, 0.0));
    app.apply_reducer_intents([GraphIntent::AssignClassification {
        key: node,
        classification: crate::model::graph::NodeClassification {
            scheme: crate::model::graph::ClassificationScheme::Udc,
            value: "udc:519.6".to_string(),
            label: Some("Computational mathematics".to_string()),
            confidence: 0.95,
            provenance: crate::model::graph::ClassificationProvenance::UserAuthored,
            status: crate::model::graph::ClassificationStatus::Accepted,
            primary: true,
        },
    }]);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    let (key, _) = reopened
        .workspace
        .domain
        .graph
        .get_node_by_url("https://persist.test/")
        .expect("node should survive restart");
    let classifications = reopened
        .workspace
        .domain
        .graph
        .node_classifications(key)
        .unwrap();
    assert_eq!(
        classifications.len(),
        1,
        "classification must survive restart"
    );
    assert_eq!(classifications[0].value, "udc:519.6");
    assert_eq!(
        classifications[0].status,
        crate::model::graph::ClassificationStatus::Accepted
    );
    assert!(classifications[0].primary);
}

// ---------------------------------------------------------------------------
// Stage C: capture and ingestion path tests
// ---------------------------------------------------------------------------

#[test]
fn test_tag_node_udc_prefix_creates_classification_with_user_authored_provenance() {
    // Spec: graph_enrichment_plan.md §Stage C done gate —
    // "UDC assignment works through label-first search" and
    // "inherited metadata is marked with provenance".
    let mut app = GraphBrowserApp::new_for_testing();
    let node = app.add_node_and_sync("https://example.com/".into(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: "udc:519.6".to_string(),
    }]);

    let classifications = app
        .workspace
        .domain
        .graph
        .node_classifications(node)
        .expect("node should exist");

    assert_eq!(
        classifications.len(),
        1,
        "one classification should be created"
    );
    let c = &classifications[0];
    assert_eq!(c.value, "udc:519.6");
    assert_eq!(
        c.scheme,
        crate::model::graph::ClassificationScheme::Udc,
        "scheme must be Udc"
    );
    assert_eq!(
        c.provenance,
        crate::model::graph::ClassificationProvenance::UserAuthored,
        "user-applied tag must carry UserAuthored provenance"
    );
    assert_eq!(
        c.status,
        crate::model::graph::ClassificationStatus::Accepted,
        "user-applied tag must be Accepted status"
    );
    assert_eq!(c.confidence, 1.0);
    // First classification on the node should be primary
    assert!(c.primary, "first classification should be primary");
}

#[test]
fn test_tag_node_udc_does_not_duplicate_classification_if_already_present() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node = app.add_node_and_sync("https://example.com/".into(), Point2D::new(0.0, 0.0));

    // Apply the same tag twice
    app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: "udc:519.6".to_string(),
    }]);
    app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: "udc:519.6".to_string(),
    }]);

    let classifications = app
        .workspace
        .domain
        .graph
        .node_classifications(node)
        .expect("node should exist");
    assert_eq!(
        classifications.len(),
        1,
        "duplicate TagNode must not duplicate classification"
    );
}

#[test]
fn test_view_policy_intents_preserve_direct_values() {
    let mut app = GraphBrowserApp::new_for_testing();

    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Test"));

    if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
        view.presentation_policy.theme = Some(ThemeData {
            background_rgb: (1, 2, 3),
            accent_rgb: (4, 5, 6),
            font_scale: 1.3,
            stroke_width: 2.0,
        });
    }

    app.apply_reducer_intents([
        GraphIntent::SetViewPhysicsProfile {
            view_id,
            profile_id: crate::registries::atomic::lens::PHYSICS_ID_GAS.to_string(),
        },
        GraphIntent::SetViewLayoutAlgorithm {
            view_id,
            algorithm_id: crate::app::graph_layout::GRAPH_LAYOUT_GRID.to_string(),
        },
    ]);

    let resolved = app.workspace.graph_runtime.views.get(&view_id).unwrap();
    assert_eq!(resolved.resolved_physics_profile().name, "Gas");
    assert!(matches!(resolved.resolved_layout_mode(), LayoutMode::Free));
    assert_eq!(
        resolved.resolved_layout_algorithm_id(),
        crate::app::graph_layout::GRAPH_LAYOUT_GRID
    );
    assert_eq!(
        resolved.resolved_theme().map(|theme| theme.background_rgb),
        Some((1, 2, 3))
    );
}

#[test]
fn test_set_view_lens_id_applies_explicit_lens_selection() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.set_default_registry_lens_id(Some("lens:default"));

    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Test"));

    app.apply_reducer_intents([GraphIntent::SetViewLensId {
        view_id,
        lens_id: crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string(),
    }]);

    let resolved = app.workspace.graph_runtime.views.get(&view_id).unwrap();
    assert_eq!(
        resolved.resolved_lens_id(),
        Some(crate::registries::atomic::lens::LENS_ID_DEFAULT)
    );
    assert_eq!(resolved.resolved_lens_display_name(), "Default");
    assert_eq!(resolved.resolved_physics_profile().name, "Liquid");
    assert!(matches!(resolved.resolved_layout_mode(), LayoutMode::Free));
    assert_eq!(
        resolved.resolved_layout_algorithm_id(),
        crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED
    );
    assert_eq!(
        resolved.resolved_theme().map(|theme| theme.background_rgb),
        Some((20, 20, 25))
    );
}

#[test]
fn toggle_semantic_depth_view_restores_previous_dimension() {
    let mut app = GraphBrowserApp::new_for_testing();

    let view_id = GraphViewId::new();
    let mut view = GraphViewState::new_with_id(view_id, "Semantic");
    view.dimension = ViewDimension::ThreeD {
        mode: ThreeDMode::Isometric,
        z_source: ZSource::BfsDepth { scale: 12.0 },
    };
    app.workspace.graph_runtime.views.insert(view_id, view);

    app.apply_reducer_intents([GraphIntent::ToggleSemanticDepthView { view_id }]);

    assert!(matches!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .dimension,
        ViewDimension::ThreeD {
            mode: ThreeDMode::TwoPointFive,
            z_source: ZSource::UdcLevel { scale: 48.0 }
        }
    ));

    app.apply_reducer_intents([GraphIntent::ToggleSemanticDepthView { view_id }]);

    assert!(matches!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .dimension,
        ViewDimension::ThreeD {
            mode: ThreeDMode::Isometric,
            z_source: ZSource::BfsDepth { scale: 12.0 }
        }
    ));
}

#[test]
fn set_view_dimension_clears_semantic_depth_restore_target() {
    let mut app = GraphBrowserApp::new_for_testing();

    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Semantic"));

    app.apply_reducer_intents([GraphIntent::ToggleSemanticDepthView { view_id }]);
    app.apply_reducer_intents([GraphIntent::SetViewDimension {
        view_id,
        dimension: ViewDimension::ThreeD {
            mode: ThreeDMode::Isometric,
            z_source: ZSource::BfsDepth { scale: 9.0 },
        },
    }]);
    app.apply_reducer_intents([GraphIntent::ToggleSemanticDepthView { view_id }]);

    assert!(matches!(
        app.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .unwrap()
            .dimension,
        ViewDimension::ThreeD {
            mode: ThreeDMode::TwoPointFive,
            z_source: ZSource::UdcLevel { scale: 48.0 }
        }
    ));
}

#[test]
fn refresh_registry_backed_view_lenses_reresolves_explicit_lens_ids_only() {
    let mut app = GraphBrowserApp::new_for_testing();

    let registry_backed_view = GraphViewId::new();
    let mut stale_registry_lens = GraphViewState::new_with_id(registry_backed_view, "Registry");
    stale_registry_lens.lens_state.display_name = "Stale".to_string();
    stale_registry_lens.lens_state.base_lens_id = Some("lens:default".to_string());
    stale_registry_lens.apply_physics_policy_override(
        crate::registries::atomic::lens::PHYSICS_ID_GAS,
        PhysicsProfile::gas(),
    );
    stale_registry_lens.apply_layout_policy_override(
        LayoutMode::Grid { gap: 42.0 },
        crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT,
    );
    stale_registry_lens.filter_policy.legacy_filters = vec!["stale".to_string()];
    app.workspace
        .graph_runtime
        .views
        .insert(registry_backed_view, stale_registry_lens);

    let direct_view = GraphViewId::new();
    let mut direct_lens_view = GraphViewState::new_with_id(direct_view, "Direct");
    direct_lens_view.lens_state.display_name = "Direct Lens".to_string();
    direct_lens_view.apply_physics_policy_override(
        crate::registries::atomic::lens::PHYSICS_ID_GAS,
        PhysicsProfile::gas(),
    );
    direct_lens_view.apply_layout_policy_override(
        LayoutMode::Grid { gap: 24.0 },
        crate::app::graph_layout::GRAPH_LAYOUT_GRID,
    );
    direct_lens_view.presentation_policy.theme = Some(ThemeData {
        background_rgb: (9, 8, 7),
        accent_rgb: (6, 5, 4),
        font_scale: 1.1,
        stroke_width: 3.0,
    });
    direct_lens_view.filter_policy.legacy_filters = vec!["custom".to_string()];
    app.workspace
        .graph_runtime
        .views
        .insert(direct_view, direct_lens_view.clone());

    let refreshed = app.refresh_registry_backed_view_lenses();
    assert_eq!(refreshed, 1);

    let registry_backed = app
        .workspace
        .graph_runtime
        .views
        .get(&registry_backed_view)
        .unwrap();
    assert_eq!(registry_backed.resolved_lens_id(), Some("lens:default"));
    assert_eq!(registry_backed.resolved_lens_display_name(), "Default");
    assert_eq!(registry_backed.resolved_physics_profile().name, "Liquid");
    assert!(matches!(registry_backed.resolved_layout_mode(), LayoutMode::Free));
    assert_eq!(
        registry_backed.resolved_layout_algorithm_id(),
        crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
    );

    let direct = app
        .workspace
        .graph_runtime
        .views
        .get(&direct_view)
        .unwrap();
    assert_eq!(direct.resolved_lens_display_name(), "Direct Lens");
    assert_eq!(direct.resolved_physics_profile().name, "Gas");
    assert!(matches!(direct.resolved_layout_mode(), LayoutMode::Grid { gap: 24.0 }));
    assert_eq!(
        direct.resolved_theme().map(|theme| theme.background_rgb),
        Some((9, 8, 7))
    );
}

// --- UpdateNodeMimeHint intent tests ---

#[test]
fn update_node_mime_hint_intent_sets_hint_on_node() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("file:///doc.pdf".to_string(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::UpdateNodeMimeHint {
        key,
        mime_hint: Some("application/pdf".to_string()),
    }]);

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
}

#[test]
fn set_zoom_updates_focused_view_camera_when_present() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
    app.workspace.graph_runtime.focused_view = Some(view_id);

    app.apply_reducer_intents([GraphIntent::SetZoom { zoom: 2.5 }]);

    assert!(
        (app.workspace.graph_runtime.views[&view_id]
            .camera
            .current_zoom
            - 2.5)
            .abs()
            < 0.0001
    );
    assert!(
        (app.workspace.graph_runtime.camera.current_zoom - Camera::new().current_zoom).abs()
            < 0.0001
    );
}

#[test]
fn set_zoom_with_missing_focused_view_is_noop() {
    let mut app = GraphBrowserApp::new_for_testing();
    let missing_view_id = GraphViewId::new();
    app.workspace.graph_runtime.focused_view = Some(missing_view_id);
    let before = app.workspace.graph_runtime.camera.current_zoom;

    app.apply_reducer_intents([GraphIntent::SetZoom { zoom: 3.0 }]);

    assert!((app.workspace.graph_runtime.camera.current_zoom - before).abs() < 0.0001);
}

#[test]
fn update_node_mime_hint_intent_can_clear_hint() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("file:///doc.pdf".to_string(), Point2D::new(0.0, 0.0));

    // Set then clear.
    app.apply_reducer_intents([GraphIntent::UpdateNodeMimeHint {
        key,
        mime_hint: Some("application/pdf".to_string()),
    }]);
    app.apply_reducer_intents([GraphIntent::UpdateNodeMimeHint {
        key,
        mime_hint: None,
    }]);

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert!(node.mime_hint.is_none());
}

#[test]
fn address_kind_is_always_derived_from_url_not_overridable_by_intent() {
    // Stage B: UpdateNodeAddressKind intent has been retired.
    // address_kind is derived from url and cannot be overridden independently.
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

    // Verify derived address_kind matches URL scheme
    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.address.address_kind(), crate::graph::AddressKind::Http);

    // After URL update, address_kind is re-derived automatically
    app.update_node_url_and_log(key, "file:///home/user/doc.txt".to_string());
    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.address.address_kind(), crate::graph::AddressKind::File);
    assert!(matches!(node.address, crate::graph::Address::File(_)));
}

#[test]
fn old_update_node_address_kind_wal_entry_is_safely_ignored() {
    // Stage B regression: legacy UpdateNodeAddressKind WAL entries must not
    // crash during replay and must leave address consistent with the URL.
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().to_path_buf();

    {
        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        let key = app.add_node_and_sync("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let node_id = app.workspace.domain.graph.get_node(key).unwrap().id;

        // Inject a legacy UpdateNodeAddressKind entry directly into the WAL.
        #[allow(deprecated)]
        app.services
            .persistence
            .as_mut()
            .expect("persistence store should exist")
            .log_mutation(
                &crate::services::persistence::types::LogEntry::UpdateNodeAddressKind {
                    node_id: node_id.to_string(),
                    kind: crate::services::persistence::types::PersistedAddressKind::Unknown,
                },
            );
    } // drop app to flush WAL to disk

    // Reload from disk — WAL replay must not panic and address_kind must be
    // re-derived from the URL (Http), not set to Unknown from the legacy entry.
    let reloaded = GraphBrowserApp::new_from_dir(path);
    let reloaded_node = reloaded
        .workspace
        .domain
        .graph
        .get_node_by_url("https://example.com")
        .map(|(_, n)| n)
        .expect("node must survive reload");
    assert_eq!(
        reloaded_node.address.address_kind(),
        crate::graph::AddressKind::Http
    );
    assert!(matches!(
        reloaded_node.address,
        crate::graph::Address::Http(_)
    ));
}

#[test]
fn node_created_with_http_url_has_http_address_kind_after_add_node() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.address.address_kind(), crate::graph::AddressKind::Http);
}

#[test]
fn node_created_with_file_pdf_url_gets_mime_hint_after_add_node() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.workspace.domain.graph.add_node(
        "file:///home/user/doc.pdf".to_string(),
        Point2D::new(0.0, 0.0),
    );
    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
    assert_eq!(node.address.address_kind(), crate::graph::AddressKind::File);
}

#[test]
fn update_node_url_and_log_refreshes_mime_hint_and_address_kind() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

    // Start with HTTP
    assert_eq!(
        app.workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .address
            .address_kind(),
        crate::graph::AddressKind::Http
    );

    // Navigate to a local PDF file
    app.update_node_url_and_log(key, "file:///home/user/report.pdf".to_string());

    let node = app.workspace.domain.graph.get_node(key).unwrap();
    assert_eq!(node.address.address_kind(), crate::graph::AddressKind::File);
    assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
}

#[test]
fn add_node_and_sync_http_url_queues_protocol_probe_request() {
    let mut app = GraphBrowserApp::new_for_testing();

    let key = app.add_node_and_sync("https://example.com".to_string(), Point2D::new(0.0, 0.0));

    assert_eq!(
        app.take_pending_protocol_probe(),
        Some((key, Some("https://example.com".to_string())))
    );
}

#[test]
fn update_node_url_and_log_to_file_url_queues_probe_cancellation() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.add_node_and_sync("https://before.example".to_string(), Point2D::new(0.0, 0.0));
    let _ = app.take_pending_protocol_probe();

    app.update_node_url_and_log(key, "file:///home/user/report.pdf".to_string());

    assert_eq!(app.take_pending_protocol_probe(), Some((key, None)));
}

#[test]
fn undo_redo_create_node_and_remove_selected_nodes() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);
    assert_eq!(app.workspace.domain.graph.node_count(), 1);
    assert_eq!(app.undo_stack_len(), 1);
    assert_eq!(app.redo_stack_len(), 0);

    app.apply_reducer_intents([GraphIntent::Undo]);
    assert_eq!(app.workspace.domain.graph.node_count(), 0);
    assert_eq!(app.undo_stack_len(), 0);
    assert_eq!(app.redo_stack_len(), 1);

    app.apply_reducer_intents([GraphIntent::Redo]);
    assert_eq!(app.workspace.domain.graph.node_count(), 1);
    assert_eq!(app.undo_stack_len(), 1);
    assert_eq!(app.redo_stack_len(), 0);

    app.apply_reducer_intents([GraphIntent::RemoveSelectedNodes]);
    assert_eq!(app.workspace.domain.graph.node_count(), 0);
    assert_eq!(app.undo_stack_len(), 2);

    app.apply_reducer_intents([GraphIntent::Undo]);
    assert_eq!(app.workspace.domain.graph.node_count(), 1);
    assert_eq!(app.redo_stack_len(), 1);
}

#[test]
fn undo_redo_set_node_url_round_trips_original_value() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://old.example".to_string(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::SetNodeUrl {
        key,
        new_url: "https://new.example".to_string(),
    }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().url(),
        "https://new.example"
    );
    assert_eq!(app.undo_stack_len(), 1);

    app.apply_reducer_intents([GraphIntent::Undo]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().url(),
        "https://old.example"
    );

    app.apply_reducer_intents([GraphIntent::Redo]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().url(),
        "https://new.example"
    );
}

#[test]
fn undo_redo_user_grouped_edge_create_and_remove_round_trip() {
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app
        .workspace
        .domain
        .graph
        .add_node("https://a.example".to_string(), Point2D::new(0.0, 0.0));
    let to = app
        .workspace
        .domain
        .graph
        .add_node("https://b.example".to_string(), Point2D::new(10.0, 0.0));

    app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from,
        to,
        label: Some("registry-label".to_string()),
    }]);
    let grouped =
        crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped);
    assert!(app.has_relation(from, to, grouped));
    let edge_key = app.workspace.domain.graph.find_edge_key(from, to).unwrap();
    let payload = app.workspace.domain.graph.get_edge(edge_key).unwrap();
    assert_eq!(payload.label(), Some("registry-label"));
    assert_eq!(app.undo_stack_len(), 1);

    app.apply_reducer_intents([GraphIntent::Undo]);
    assert!(!app.has_relation(from, to, grouped));

    app.apply_reducer_intents([GraphIntent::Redo]);
    assert!(app.has_relation(from, to, grouped));

    app.apply_reducer_intents([GraphIntent::RemoveEdge {
        from,
        to,
        selector: crate::graph::RelationSelector::Semantic(
            crate::graph::SemanticSubKind::UserGrouped,
        ),
    }]);
    assert!(!app.has_relation(from, to, grouped));
    assert_eq!(app.undo_stack_len(), 2);

    app.apply_reducer_intents([GraphIntent::Undo]);
    assert!(app.has_relation(from, to, grouped));
}

#[test]
fn undo_redo_queue_history_frame_layout_restore_requests() {
    let mut app = GraphBrowserApp::new_for_testing();
    let before_layout = "{\"frame\":\"before\"}";
    let after_layout = "{\"frame\":\"after\"}";

    app.mark_session_frame_layout_json(before_layout);
    app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);
    app.mark_session_frame_layout_json(after_layout);

    app.apply_reducer_intents([GraphIntent::Undo]);
    assert_eq!(
        app.take_pending_history_frame_layout_json(),
        Some(before_layout.to_string())
    );

    app.mark_session_frame_layout_json(before_layout);
    app.apply_reducer_intents([GraphIntent::Redo]);
    assert_eq!(
        app.take_pending_history_frame_layout_json(),
        Some(after_layout.to_string())
    );
}

#[test]
fn set_node_url_noop_does_not_capture_undo_checkpoint() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .domain
        .graph
        .add_node("https://same.example".to_string(), Point2D::new(0.0, 0.0));

    app.apply_reducer_intents([GraphIntent::SetNodeUrl {
        key,
        new_url: "https://same.example".to_string(),
    }]);

    assert_eq!(app.undo_stack_len(), 0);
    assert_eq!(app.redo_stack_len(), 0);
}

#[cfg(feature = "diagnostics")]
#[test]
fn stale_camera_target_enqueue_emits_blocked_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let stale_target = GraphViewId::new();
    app.clear_pending_camera_command();

    app.request_camera_command_for_view(Some(stale_target), CameraCommand::Fit);

    assert!(app.pending_camera_command().is_none());
    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("runtime.ui.graph.camera_command_blocked_missing_target_view"),
        "expected stale camera target enqueue to emit blocked channel"
    );
}

#[test]
fn pending_workbench_intent_queue_is_explicit_and_drainable() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.enqueue_workbench_intent(WorkbenchIntent::CycleFocusRegion);
    assert_eq!(app.pending_workbench_intent_count_for_tests(), 1);

    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [WorkbenchIntent::CycleFocusRegion]
    ));
    assert_eq!(app.pending_workbench_intent_count_for_tests(), 0);
}

#[test]
fn reducer_pane_presentation_and_promotion_flow_through_workbench_intents() {
    let mut app = GraphBrowserApp::new_for_testing();
    let pane = crate::shell::desktop::workbench::pane_model::PaneId::new();

    app.apply_reducer_intents([
        GraphIntent::SetPanePresentationMode {
            pane,
            mode: crate::shell::desktop::workbench::pane_model::PanePresentationMode::Docked,
        },
        GraphIntent::PromoteEphemeralPane {
            target_tile_context:
                crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::Split,
        },
    ]);

    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [
            WorkbenchIntent::SetPanePresentationMode {
                pane: drained_pane,
                mode: crate::shell::desktop::workbench::pane_model::PanePresentationMode::Docked,
            },
            WorkbenchIntent::PromoteEphemeralPane {
                target_tile_context:
                    crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::Split,
            }
        ] if *drained_pane == pane
    ));
}

#[test]
fn workbench_tile_selection_update_modes_track_primary_tile() {
    let mut app = GraphBrowserApp::new_for_testing();
    let mut tiles = egui_tiles::Tiles::default();
    let tile_a = tiles.insert_pane(
        crate::shell::desktop::workbench::tile_kind::TileKind::Graph(
            crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::new()),
        ),
    );
    let tile_b = tiles.insert_pane(
        crate::shell::desktop::workbench::tile_kind::TileKind::Graph(
            crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::new()),
        ),
    );

    app.update_workbench_tile_selection(tile_a, SelectionUpdateMode::Replace);
    assert_eq!(
        app.workbench_tile_selection().selected_tile_ids,
        HashSet::from([tile_a])
    );
    assert_eq!(app.workbench_tile_selection().primary_tile_id, Some(tile_a));

    app.update_workbench_tile_selection(tile_b, SelectionUpdateMode::Add);
    assert_eq!(
        app.workbench_tile_selection().selected_tile_ids,
        HashSet::from([tile_a, tile_b])
    );
    assert_eq!(app.workbench_tile_selection().primary_tile_id, Some(tile_b));

    app.update_workbench_tile_selection(tile_b, SelectionUpdateMode::Toggle);
    assert_eq!(
        app.workbench_tile_selection().selected_tile_ids,
        HashSet::from([tile_a])
    );
    assert_eq!(app.workbench_tile_selection().primary_tile_id, Some(tile_a));
}

#[test]
fn prune_workbench_tile_selection_discards_stale_tile_ids() {
    let mut app = GraphBrowserApp::new_for_testing();
    let mut tiles = egui_tiles::Tiles::default();
    let tile = tiles.insert_pane(
        crate::shell::desktop::workbench::tile_kind::TileKind::Graph(
            crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::new()),
        ),
    );
    let root = tiles.insert_tab_tile(vec![tile]);
    let mut tree = egui_tiles::Tree::new("prune_workbench_tile_selection", root, tiles);

    app.select_workbench_tile(tile);
    tree.remove_recursively(tile);
    app.prune_workbench_tile_selection(&tree);

    assert!(app.workbench_tile_selection().selected_tile_ids.is_empty());
    assert_eq!(app.workbench_tile_selection().primary_tile_id, None);
}

#[test]
fn detach_node_to_split_requests_flow_through_workbench_intents() {
    let mut app = GraphBrowserApp::new_for_testing();
    let key = NodeKey::new(7);

    app.request_detach_node_to_split(key);

    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [WorkbenchIntent::DetachNodeToSplit { key: drained_key }] if *drained_key == key
    ));
}

#[test]
fn workbench_intents_do_not_bypass_reducer_mutation_entry() {
    let mut app = GraphBrowserApp::new_for_testing();
    let before_count = app.workspace.domain.graph.node_count();

    app.enqueue_workbench_intent(WorkbenchIntent::OpenGraphUrl {
        url: GraphAddress::graph("missing-graph").to_string(),
    });

    assert_eq!(
        app.workspace.domain.graph.node_count(),
        before_count,
        "enqueuing a workbench intent must not mutate reducer-owned graph state"
    );

    app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert_eq!(
        app.workspace.domain.graph.node_count(),
        before_count + 1,
        "graph mutation must flow through apply_reducer_intents"
    );
}

#[test]
fn graph_view_layout_manager_entry_exit_and_toggle_intents_update_state() {
    let mut app = GraphBrowserApp::new_for_testing();
    assert!(!app.graph_view_layout_manager_active());

    app.apply_reducer_intents([GraphIntent::EnterGraphViewLayoutManager]);
    assert!(app.graph_view_layout_manager_active());

    app.apply_reducer_intents([GraphIntent::ExitGraphViewLayoutManager]);
    assert!(!app.graph_view_layout_manager_active());

    app.apply_reducer_intents([GraphIntent::ToggleGraphViewLayoutManager]);
    assert!(app.graph_view_layout_manager_active());
}

#[test]
fn graph_view_slot_lifecycle_create_rename_move_archive_restore() {
    let mut app = GraphBrowserApp::new_for_testing();
    let anchor = GraphViewId::new();
    app.ensure_graph_view_registered(anchor);

    app.apply_reducer_intents([GraphIntent::CreateGraphViewSlot {
        anchor_view: Some(anchor),
        direction: GraphViewLayoutDirection::Right,
        open_mode: None,
    }]);

    let mut slots = app.graph_view_slots_for_tests();
    assert_eq!(slots.len(), 2);
    let created = slots
        .iter()
        .find(|slot| slot.view_id != anchor)
        .expect("expected created graph-view slot")
        .view_id;

    app.apply_reducer_intents([GraphIntent::RenameGraphViewSlot {
        view_id: created,
        name: "Investigation View".to_string(),
    }]);
    slots = app.graph_view_slots_for_tests();
    assert!(
        slots
            .iter()
            .any(|slot| slot.view_id == created && slot.name == "Investigation View")
    );

    app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
        view_id: created,
        row: 3,
        col: 2,
    }]);
    slots = app.graph_view_slots_for_tests();
    assert!(
        slots
            .iter()
            .any(|slot| slot.view_id == created && slot.row == 3 && slot.col == 2)
    );

    app.apply_reducer_intents([GraphIntent::ArchiveGraphViewSlot { view_id: created }]);
    slots = app.graph_view_slots_for_tests();
    assert!(
        slots
            .iter()
            .any(|slot| slot.view_id == created && slot.archived)
    );

    app.apply_reducer_intents([GraphIntent::RestoreGraphViewSlot {
        view_id: created,
        row: 4,
        col: 4,
    }]);
    slots = app.graph_view_slots_for_tests();
    assert!(slots.iter().any(|slot| {
        slot.view_id == created && !slot.archived && slot.row == 4 && slot.col == 4
    }));
}

#[test]
fn graph_view_slot_move_guard_prevents_coordinate_collision() {
    let mut app = GraphBrowserApp::new_for_testing();
    let left = GraphViewId::new();
    let right = GraphViewId::new();
    app.ensure_graph_view_registered(left);
    app.ensure_graph_view_registered(right);

    app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
        view_id: left,
        row: 1,
        col: 1,
    }]);
    app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
        view_id: right,
        row: 2,
        col: 2,
    }]);

    app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
        view_id: right,
        row: 1,
        col: 1,
    }]);

    let slots = app.graph_view_slots_for_tests();
    let right_slot = slots
        .iter()
        .find(|slot| slot.view_id == right)
        .expect("right slot should exist");
    assert_eq!(
        (right_slot.row, right_slot.col),
        (2, 2),
        "move into occupied slot should be rejected"
    );
}

#[test]
fn route_graph_view_to_workbench_enqueues_open_graph_view_pane_intent() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.ensure_graph_view_registered(view_id);

    app.apply_reducer_intents([GraphIntent::RouteGraphViewToWorkbench {
        view_id,
        mode: PendingTileOpenMode::SplitHorizontal,
    }]);

    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [WorkbenchIntent::OpenGraphViewPane {
            view_id: routed,
            mode: PendingTileOpenMode::SplitHorizontal
        }] if *routed == view_id
    ));
}

#[test]
fn workbench_authority_bridge_intent_is_classified_for_reducer_warning() {
    assert_eq!(
        GraphIntent::RouteGraphViewToWorkbench {
            view_id: GraphViewId::new(),
            mode: PendingTileOpenMode::SplitHorizontal,
        }
        .workbench_authority_bridge_name(),
        Some("RouteGraphViewToWorkbench")
    );
    assert_eq!(
        GraphIntent::CreateNodeNearCenter.workbench_authority_bridge_name(),
        None
    );
}

#[test]
fn persisted_graph_view_layout_manager_shape_round_trips() {
    let view_id = GraphViewId::new();
    let persisted = PersistedGraphViewLayoutManager {
        version: PersistedGraphViewLayoutManager::VERSION,
        active: true,
        slots: vec![GraphViewSlot {
            view_id,
            name: "Primary".to_string(),
            row: 0,
            col: 1,
            archived: false,
        }],
    };

    let json = serde_json::to_string(&persisted).expect("persisted manager should serialize");
    let decoded: PersistedGraphViewLayoutManager =
        serde_json::from_str(&json).expect("persisted manager should deserialize");

    assert_eq!(decoded.version, PersistedGraphViewLayoutManager::VERSION);
    assert!(decoded.active);
    assert_eq!(decoded.slots.len(), 1);
    assert_eq!(decoded.slots[0].view_id, view_id);
    assert_eq!(decoded.slots[0].name, "Primary");
}

#[cfg(feature = "diagnostics")]
#[test]
fn toggle_command_palette_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    assert!(!app.workspace.chrome_ui.show_command_palette);

    app.toggle_command_palette();

    assert!(app.workspace.chrome_ui.show_command_palette);
    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when command palette focus surface toggles"
    );
}

#[test]
fn toggle_help_panel_reducer_path_enqueues_workbench_intent() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::ToggleHelpPanel]);

    assert!(!app.workspace.chrome_ui.show_help_panel);
    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [WorkbenchIntent::ToggleHelpPanel]
    ));
}

#[test]
fn toggle_radial_menu_reducer_path_enqueues_workbench_intent() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::ToggleRadialMenu]);

    assert!(!app.workspace.chrome_ui.show_radial_menu);
    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [WorkbenchIntent::ToggleRadialMenu]
    ));
}

#[cfg(feature = "diagnostics")]
#[test]
fn set_navigator_selected_rows_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::SetNavigatorSelectedRows {
        rows: vec!["row:test".to_string()],
    }]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when file tree selected rows change"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn clear_graph_focused_view_reset_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.graph_runtime.focused_view = Some(GraphViewId::new());

    app.clear_graph();

    assert!(app.workspace.graph_runtime.focused_view.is_none());
    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when clear_graph resets focused view"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn set_highlighted_edge_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
    let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));

    app.apply_reducer_intents([GraphIntent::SetHighlightedEdge { from, to }]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when edge highlight focus changes"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn webview_url_changed_emits_history_traversal_recorded_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let a = app
        .workspace
        .domain
        .graph
        .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
    let _b = app
        .workspace
        .domain
        .graph
        .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, a);

    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://b.com".into(),
    }]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("history.traversal.recorded"),
        "expected history.traversal.recorded when traversal append succeeds"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn remove_history_edge_emits_history_archive_dissolved_appended_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let dir = TempDir::new().expect("temp dir");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());

    let a = app.add_node_and_sync("https://a.com".to_string(), Point2D::new(0.0, 0.0));
    let b = app.add_node_and_sync("https://b.com".to_string(), Point2D::new(100.0, 0.0));
    let wv = test_webview_id();
    app.map_webview_to_node(wv, a);
    app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
        webview_id: wv,
        new_url: "https://b.com".into(),
    }]);

    app.apply_reducer_intents([GraphIntent::RemoveEdge {
        from: a,
        to: b,
        selector: crate::graph::RelationSelector::Family(crate::graph::EdgeFamily::Traversal),
    }]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("history.archive.dissolved_appended"),
        "expected history.archive.dissolved_appended when dissolution archive receives entries"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn clear_and_export_history_without_persistence_emit_failure_channels() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([
        GraphIntent::ClearHistoryTimeline,
        GraphIntent::ClearHistoryDissolved,
        GraphIntent::ExportHistoryTimeline,
        GraphIntent::ExportHistoryDissolved,
    ]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("history.archive.clear_failed"),
        "expected history.archive.clear_failed when clear is requested without persistence"
    );
    assert!(
        snapshot.contains("history.archive.export_failed"),
        "expected history.archive.export_failed when export is requested without persistence"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn history_preview_and_replay_intents_emit_timeline_channels() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([
        GraphIntent::EnterHistoryTimelinePreview,
        GraphIntent::HistoryTimelinePreviewIsolationViolation {
            detail: "forbidden side effect".to_string(),
        },
        GraphIntent::HistoryTimelineReplayStarted,
        GraphIntent::HistoryTimelineReplayFinished {
            succeeded: true,
            error: None,
        },
        GraphIntent::HistoryTimelineReplayFinished {
            succeeded: false,
            error: Some("replay checksum mismatch".to_string()),
        },
        GraphIntent::ExitHistoryTimelinePreview,
        GraphIntent::HistoryTimelineReturnToPresentFailed {
            detail: "state restore mismatch".to_string(),
        },
    ]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    for channel in [
        "history.timeline.preview_entered",
        "history.timeline.preview_exited",
        "history.timeline.preview_isolation_violation",
        "history.timeline.replay_started",
        "history.timeline.replay_succeeded",
        "history.timeline.replay_failed",
        "history.timeline.return_to_present_failed",
    ] {
        assert!(
            snapshot.contains(channel),
            "expected diagnostics snapshot to contain {channel}"
        );
    }
}

#[cfg(feature = "diagnostics")]
#[test]
fn clear_highlighted_edge_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
    let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));
    app.workspace.graph_runtime.highlighted_graph_edge = Some((from, to));

    app.apply_reducer_intents([GraphIntent::ClearHighlightedEdge]);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when edge highlight focus clears"
    );
}

#[test]
fn resolve_clip_route_accepts_both_canonical_and_legacy_internal_schemes() {
    assert_eq!(
        GraphBrowserApp::resolve_clip_route("verso://clip/clip-a").as_deref(),
        Some("clip-a")
    );
    assert_eq!(
        GraphBrowserApp::resolve_clip_route("graphshell://clip/clip-b").as_deref(),
        Some("clip-b")
    );
    assert!(GraphBrowserApp::resolve_clip_route("verso://clip").is_none());
}

#[test]
fn pending_clip_request_queue_roundtrips_single_value() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.request_open_clip_by_id("clip-roundtrip");

    assert_eq!(
        app.take_pending_open_clip_request().as_deref(),
        Some("clip-roundtrip")
    );
    assert!(app.take_pending_open_clip_request().is_none());
}

#[test]
fn queued_node_context_target_supports_replace_peek_and_clear() {
    let mut app = GraphBrowserApp::new_for_testing();
    let first = NodeKey::new(9);
    let second = NodeKey::new(10);

    app.set_pending_node_context_target(Some(first));
    app.set_pending_node_context_target(Some(second));

    assert_eq!(app.pending_node_context_target(), Some(second));

    app.set_pending_node_context_target(None);
    assert!(app.pending_node_context_target().is_none());
}

#[test]
fn queued_choose_frame_picker_supports_replace_clear_and_sanitization() {
    use euclid::default::Point2D;

    let mut app = GraphBrowserApp::new_for_testing();
    let anchor = app.add_node_and_sync("https://anchor.example".into(), Point2D::new(0.0, 0.0));
    let sibling = app.add_node_and_sync("https://sibling.example".into(), Point2D::new(20.0, 0.0));

    app.request_choose_frame_picker_for_mode(anchor, ChooseFramePickerMode::AddNodeToFrame);
    assert_eq!(
        app.choose_frame_picker_request(),
        Some(ChooseFramePickerRequest {
            node: anchor,
            mode: ChooseFramePickerMode::AddNodeToFrame,
        })
    );
    assert!(app.choose_frame_picker_exact_nodes().is_none());

    app.request_add_exact_selection_to_frame_picker(vec![sibling, anchor]);
    assert_eq!(
        app.choose_frame_picker_request(),
        Some(ChooseFramePickerRequest {
            node: anchor,
            mode: ChooseFramePickerMode::AddExactSelectionToFrame,
        })
    );
    assert_eq!(
        app.choose_frame_picker_exact_nodes(),
        Some(&[anchor, sibling][..])
    );

    app.select_node(sibling, false);
    app.apply_reducer_intents([GraphIntent::RemoveSelectedNodes]);

    assert_eq!(
        app.choose_frame_picker_request(),
        Some(ChooseFramePickerRequest {
            node: anchor,
            mode: ChooseFramePickerMode::AddExactSelectionToFrame,
        })
    );
    assert_eq!(app.choose_frame_picker_exact_nodes(), Some(&[anchor][..]));

    app.clear_choose_frame_picker();
    assert!(app.choose_frame_picker_request().is_none());
    assert!(app.choose_frame_picker_exact_nodes().is_none());
}

#[test]
fn queued_unsaved_frame_prompt_supports_replace_action_and_resolution() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.request_unsaved_frame_prompt(UnsavedFramePromptRequest::FrameSwitch {
        name: "workspace-a".to_string(),
        focus_node: Some(NodeKey::new(7)),
    });
    app.request_unsaved_frame_prompt(UnsavedFramePromptRequest::FrameSwitch {
        name: "workspace-b".to_string(),
        focus_node: Some(NodeKey::new(8)),
    });

    assert_eq!(
        app.unsaved_frame_prompt_request(),
        Some(&UnsavedFramePromptRequest::FrameSwitch {
            name: "workspace-b".to_string(),
            focus_node: Some(NodeKey::new(8)),
        })
    );

    app.set_unsaved_frame_prompt_action(UnsavedFramePromptAction::ProceedWithoutSaving);

    assert_eq!(
        app.take_unsaved_frame_prompt_resolution(),
        Some((
            UnsavedFramePromptRequest::FrameSwitch {
                name: "workspace-b".to_string(),
                focus_node: Some(NodeKey::new(8)),
            },
            UnsavedFramePromptAction::ProceedWithoutSaving,
        ))
    );
    assert!(app.unsaved_frame_prompt_request().is_none());
    assert!(app.take_unsaved_frame_prompt_resolution().is_none());
}

#[test]
fn resolve_graph_route_accepts_graph_scheme() {
    assert_eq!(
        GraphBrowserApp::resolve_graph_route("graph://graph-main").as_deref(),
        Some("graph-main")
    );
    assert!(GraphBrowserApp::resolve_graph_route("graph://").is_none());
}

#[test]
fn resolve_node_route_accepts_node_scheme_with_uuid() {
    let node_id = Uuid::new_v4();
    let route = format!("node://{}", node_id);
    assert_eq!(GraphBrowserApp::resolve_node_route(&route), Some(node_id));
    assert!(GraphBrowserApp::resolve_node_route("node://not-a-uuid").is_none());
}

#[test]
fn resolve_view_route_accepts_graph_target_variant() {
    let route = GraphBrowserApp::resolve_view_route("verso://view/graph/graph-main")
        .expect("view graph route should parse");
    assert!(matches!(
        route,
        ViewRouteTarget::Graph(graph_id) if graph_id == "graph-main"
    ));
}

#[test]
fn resolve_view_route_accepts_node_target_variant() {
    let node_id = Uuid::new_v4();
    let route =
        GraphBrowserApp::resolve_view_route(format!("verso://view/node/{node_id}").as_str())
            .expect("view node route should parse");
    assert!(matches!(route, ViewRouteTarget::Node(parsed) if parsed == node_id));
}

#[test]
fn resolve_view_route_accepts_note_target_variant() {
    let note_id = Uuid::new_v4();
    let route =
        GraphBrowserApp::resolve_view_route(format!("verso://view/note/{note_id}").as_str())
            .expect("view note route should parse");
    assert!(matches!(
        route,
        ViewRouteTarget::Note(parsed) if parsed.as_uuid() == note_id
    ));
}

#[test]
fn opening_help_panel_closes_other_capture_surfaces() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.chrome_ui.show_command_palette = true;
    app.workspace.chrome_ui.show_context_palette = true;
    app.workspace.chrome_ui.command_palette_contextual_mode = true;
    app.workspace.chrome_ui.show_radial_menu = true;
    app.set_pending_node_context_target(Some(NodeKey::new(9)));

    app.open_help_panel();

    assert!(app.workspace.chrome_ui.show_help_panel);
    assert!(!app.workspace.chrome_ui.show_command_palette);
    assert!(!app.workspace.chrome_ui.show_context_palette);
    assert!(!app.workspace.chrome_ui.command_palette_contextual_mode);
    assert!(!app.workspace.chrome_ui.show_radial_menu);
    assert!(app.pending_node_context_target().is_none());
}

#[test]
fn opening_command_palette_closes_other_capture_surfaces() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.chrome_ui.show_help_panel = true;
    app.workspace.chrome_ui.show_radial_menu = true;
    app.set_pending_node_context_target(Some(NodeKey::new(10)));

    app.open_command_palette();

    assert!(app.workspace.chrome_ui.show_command_palette);
    assert!(!app.workspace.chrome_ui.show_context_palette);
    assert!(!app.workspace.chrome_ui.command_palette_contextual_mode);
    assert!(!app.workspace.chrome_ui.show_help_panel);
    assert!(!app.workspace.chrome_ui.show_radial_menu);
    assert!(app.pending_node_context_target().is_none());
}

#[test]
fn opening_context_palette_preserves_node_context_until_close() {
    let mut app = GraphBrowserApp::new_for_testing();
    let target = NodeKey::new(11);
    app.set_pending_node_context_target(Some(target));

    app.open_context_palette();

    assert!(!app.workspace.chrome_ui.show_command_palette);
    assert!(app.workspace.chrome_ui.show_context_palette);
    assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
    assert_eq!(app.pending_node_context_target(), Some(target));

    app.close_command_palette();

    assert!(!app.workspace.chrome_ui.show_command_palette);
    assert!(!app.workspace.chrome_ui.show_context_palette);
    assert!(!app.workspace.chrome_ui.command_palette_contextual_mode);
    assert!(app.pending_node_context_target().is_none());
}

#[test]
fn toggle_command_palette_reducer_path_enqueues_workbench_intent() {
    let mut app = GraphBrowserApp::new_for_testing();

    app.apply_reducer_intents([GraphIntent::ToggleCommandPalette]);

    assert!(!app.workspace.chrome_ui.show_command_palette);
    let drained = app.take_pending_workbench_intents();
    assert!(matches!(
        drained.as_slice(),
        [WorkbenchIntent::ToggleCommandPalette]
    ));
}

#[test]
fn opening_radial_menu_closes_other_capture_surfaces() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.chrome_ui.show_help_panel = true;
    app.workspace.chrome_ui.show_command_palette = true;
    app.workspace.chrome_ui.show_context_palette = true;

    app.open_radial_menu();

    assert!(app.workspace.chrome_ui.show_radial_menu);
    assert!(!app.workspace.chrome_ui.show_help_panel);
    assert!(!app.workspace.chrome_ui.show_command_palette);
    assert!(!app.workspace.chrome_ui.show_context_palette);
}

// ── Ghost Node (Tombstone lifecycle) scenario tests ───────────────────────────

#[test]
fn mark_tombstone_transitions_selected_node_to_tombstone() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.workspace.domain.graph.add_node(
        "ghost-me".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );

    // Select then ghost.
    app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);
    app.apply_reducer_intents([GraphIntent::MarkTombstoneForSelected]);

    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Tombstone,
        "node should be Tombstone after MarkTombstoneForSelected"
    );
    // Selection must be cleared after tombstoning.
    assert!(
        app.focused_selection().is_empty(),
        "selection should be cleared after tombstoning"
    );
}

#[test]
fn restore_ghost_node_transitions_tombstone_to_cold() {
    use crate::graph::NodeLifecycle;

    let mut app = GraphBrowserApp::new_for_testing();
    let key = app.workspace.domain.graph.add_node(
        "restore-me".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );

    // Ghost the node first.
    app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);
    app.apply_reducer_intents([GraphIntent::MarkTombstoneForSelected]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Tombstone
    );

    // Restore it.
    app.apply_reducer_intents([GraphIntent::RestoreGhostNode { key }]);
    assert_eq!(
        app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
        NodeLifecycle::Cold,
        "node should be Cold after RestoreGhostNode"
    );
}

#[test]
fn toggle_ghost_nodes_flips_tombstones_visible_on_focused_view() {
    use crate::app::GraphViewState;

    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    app.workspace
        .graph_runtime
        .views
        .insert(view_id, GraphViewState::new_with_id(view_id, "TestView"));
    app.set_workspace_focused_view_with_transition(Some(view_id));

    let initial = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.tombstones_visible)
        .unwrap_or(false);

    app.apply_reducer_intents([GraphIntent::ToggleGhostNodes]);

    let after_toggle = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|v| v.tombstones_visible)
        .unwrap_or(false);

    assert_ne!(
        initial, after_toggle,
        "ToggleGhostNodes should flip tombstones_visible"
    );
}

#[test]
fn set_navigator_specialty_view_creates_view_with_graphlet_mask() {
    use crate::app::workbench_layout_policy::{NavigatorHostId, SurfaceHostId};
    use crate::graph::GraphletKind;

    let mut app = GraphBrowserApp::new_for_testing();
    // Add two nodes and select one as the primary selection anchor.
    let anchor = app.add_node_and_sync("https://a.example/".to_string(), Point2D::new(0.0, 0.0));
    let _peer = app.add_node_and_sync("https://b.example/".to_string(), Point2D::new(1.0, 0.0));
    app.apply_reducer_intents([GraphIntent::SelectNode {
        key: anchor,
        multi_select: false,
    }]);

    let host = SurfaceHostId::Navigator(NavigatorHostId::Left);
    app.apply_reducer_intents([GraphIntent::SetNavigatorSpecialtyView {
        host: host.clone(),
        kind: Some(GraphletKind::Ego { radius: 1 }),
    }]);

    let sv = app
        .workspace
        .workbench_session
        .navigator_specialty_views
        .get(&host)
        .cloned();
    assert!(sv.is_some(), "specialty view should be registered for host");
    let sv = sv.unwrap();
    assert!(
        matches!(sv.kind, GraphletKind::Ego { .. }),
        "specialty view kind should be Ego"
    );
    let mask = app
        .workspace
        .graph_runtime
        .views
        .get(&sv.view_id)
        .and_then(|v| v.graphlet_node_mask.as_ref());
    assert!(
        mask.is_some(),
        "graphlet_node_mask should be set on the view"
    );
    assert!(
        mask.unwrap().contains(&anchor),
        "anchor node should be in the graphlet mask"
    );
}

#[test]
fn clear_navigator_specialty_view_removes_entry_and_mask() {
    use crate::app::workbench_layout_policy::{NavigatorHostId, SurfaceHostId};
    use crate::graph::GraphletKind;

    let mut app = GraphBrowserApp::new_for_testing();
    let anchor = app.add_node_and_sync("https://c.example/".to_string(), Point2D::new(0.0, 0.0));
    app.apply_reducer_intents([GraphIntent::SelectNode {
        key: anchor,
        multi_select: false,
    }]);

    let host = SurfaceHostId::Navigator(NavigatorHostId::Left);
    app.apply_reducer_intents([GraphIntent::SetNavigatorSpecialtyView {
        host: host.clone(),
        kind: Some(GraphletKind::Ego { radius: 1 }),
    }]);
    let view_id = app
        .workspace
        .workbench_session
        .navigator_specialty_views
        .get(&host)
        .map(|sv| sv.view_id)
        .expect("specialty view should exist before clearing");

    app.apply_reducer_intents([GraphIntent::SetNavigatorSpecialtyView {
        host: host.clone(),
        kind: None,
    }]);

    assert!(
        !app.workspace
            .workbench_session
            .navigator_specialty_views
            .contains_key(&host),
        "specialty view entry should be removed after clearing"
    );
    let mask_after_clear = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .and_then(|v| v.graphlet_node_mask.as_ref());
    assert!(
        mask_after_clear.is_none(),
        "graphlet_node_mask should be cleared from the view"
    );
}
