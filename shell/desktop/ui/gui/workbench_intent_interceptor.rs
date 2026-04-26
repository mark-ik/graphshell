use super::workbench_dispatch_flow::{
    UxDispatchPhase, assert_workbench_intents_drained_before_reducer_apply,
    dispatch_workbench_authority_intent, emit_dispatch_phase, modal_allows_workbench_intent,
    prime_runtime_focus_authority_for_workbench_intent,
    reconcile_focus_authority_after_realization,
    refresh_runtime_focus_authority_after_workbench_intent, ux_dispatch_path_for_workbench_intent,
    ux_event_kind_for_workbench_intent,
};
use super::*;

pub(super) fn handle_tool_pane_intents_with_modal_state_and_focus_authority(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    mut graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    workbench_intents: &mut Vec<WorkbenchIntent>,
    modal_surface_active: bool,
    mut focus_authority: Option<&mut RuntimeFocusAuthorityState>,
) {
    let mut remaining = Vec::with_capacity(workbench_intents.len());
    for intent in workbench_intents.drain(..) {
        if let Some(authority) = focus_authority.as_deref_mut() {
            prime_runtime_focus_authority_for_workbench_intent(
                authority, graph_app, tiles_tree, &intent,
            );
        }
        let event_kind = ux_event_kind_for_workbench_intent(&intent);
        let path = ux_dispatch_path_for_workbench_intent(&intent);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_DISPATCH_STARTED,
            byte_len: event_kind as usize,
        });

        if !path.is_valid() {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
                latency_us: 0,
            });
            remaining.push(intent);
            continue;
        }

        emit_dispatch_phase(UxDispatchPhase::Capture);
        let modal_focus_authority = focus_authority.as_deref();
        if modal_surface_active
            && !modal_allows_workbench_intent(graph_app, &intent, modal_focus_authority)
        {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_CONSUMED,
                byte_len: path.nodes.len(),
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED,
                byte_len: 1,
            });
            if let Some(authority) = focus_authority.as_deref_mut() {
                refresh_runtime_focus_authority_after_workbench_intent(
                    authority,
                    graph_app,
                    tiles_tree,
                    modal_surface_active,
                );
            }
            continue;
        }

        emit_dispatch_phase(UxDispatchPhase::Target);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_DISPATCH_PHASE,
            byte_len: UxDispatchPhase::Target as usize,
        });

        // Phase D: graph_tree is threaded to the authority intent dispatch so that
        // open/activate/dismiss commands update both egui_tiles and GraphTree.
        // FocusRealizer receives None — it handles UI-focus intents, not graph-node
        // topology changes, and will be updated in a follow-on pass.
        let authority_result = if let Some(authority) = focus_authority.as_deref_mut() {
            let mut realizer = FocusRealizer::new(graph_app, tiles_tree);
            realizer.realize_workbench_intent(authority, &intent)
        } else {
            dispatch_workbench_authority_intent(
                graph_app,
                tiles_tree,
                graph_tree.as_deref_mut(),
                intent.clone(),
            )
        };
        let authority_handled = authority_result.is_none();

        if authority_handled {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_CONSUMED,
                byte_len: 1,
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED,
                byte_len: 1,
            });
        } else if let Some(unhandled) = authority_result {
            emit_dispatch_phase(UxDispatchPhase::Bubble);
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_CONTRACT_WARNING,
                byte_len: 1,
            });
            emit_dispatch_phase(UxDispatchPhase::Default);
            remaining.push(unhandled);
        } else {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_CONSUMED,
                byte_len: 1,
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED,
                byte_len: 1,
            });
        }
        if let Some(authority) = focus_authority.as_deref_mut() {
            if authority_handled {
                reconcile_focus_authority_after_realization(
                    authority,
                    graph_app,
                    tiles_tree,
                    modal_surface_active,
                );
            } else {
                refresh_runtime_focus_authority_after_workbench_intent(
                    authority,
                    graph_app,
                    tiles_tree,
                    modal_surface_active,
                );
            }
        }
    }
    *workbench_intents = remaining;
}

pub(super) fn apply_semantic_intents_and_pending_open(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    modal_surface_active: bool,
    focus_authority: &mut RuntimeFocusAuthorityState,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let mut workbench_intents = graph_app.take_pending_workbench_intents();
    let mut graph_tree = graph_tree;
    handle_tool_pane_intents_with_modal_state_and_focus_authority(
        graph_app,
        tiles_tree,
        graph_tree.as_deref_mut(),
        &mut workbench_intents,
        modal_surface_active,
        Some(focus_authority),
    );
    assert_workbench_intents_drained_before_reducer_apply(&workbench_intents);
    gui_frame::apply_intents_if_any(graph_app, tiles_tree, frame_intents);
    handle_pending_open_node_after_intents(
        graph_app,
        tiles_tree,
        graph_tree.as_deref_mut(),
        open_node_tile_after_intents,
        frame_intents,
    );
    restore_pending_transient_surface_focus(graph_app, tiles_tree, focus_authority);
    handle_pending_open_note_after_intents(graph_app, tiles_tree, graph_tree.as_deref_mut());
    handle_pending_open_clip_after_intents(graph_app, tiles_tree, graph_tree.as_deref_mut());
}

pub(super) fn restore_pending_transient_surface_focus(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_authority: &mut RuntimeFocusAuthorityState,
) {
    let mut realizer = FocusRealizer::new(graph_app, tiles_tree);
    realizer.restore_pending_transient_surface_focus(focus_authority);
}
