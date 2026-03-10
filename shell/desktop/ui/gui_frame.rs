/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};
use euclid::Length;
use log::{debug, warn};
use servo::{DeviceIndependentPixel, OffscreenRenderingContext, WebViewId, WindowRenderingContext};

use super::nav_targeting;
use super::undo_boundary::record_workspace_undo_boundary_from_tiles_tree;
use crate::app::{
    BrowserCommand, BrowserCommandTarget, GraphBrowserApp, GraphIntent, GraphViewId,
    LifecycleCause, PendingConnectedOpenScope,
    PendingNodeOpenRequest, PendingTileOpenMode, ReducerDispatchContext, UndoBoundaryReason,
    UnsavedFramePromptAction, UnsavedFramePromptRequest,
};
use crate::graph::NodeKey;
use crate::render;
use crate::shell::desktop::host::headed_window::HeadedWindow;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::lifecycle_reconcile::{
    self, ActivePrewarmArgs, RuntimeReconcileArgs,
};
use crate::shell::desktop::lifecycle::semantic_event_pipeline;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::registries::{
    self, CHANNEL_UX_NAVIGATION_TRANSITION,
};
use crate::shell::desktop::ui::persistence_ops;
use crate::shell::desktop::ui::thumbnail_pipeline;
use crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailCaptureResult;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::tile_invariants;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_render_pass::{self, TileRenderPassArgs};
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops;
#[cfg(all(
    feature = "gamepad",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::host::gamepad::GamepadUiCommand;
#[cfg(all(
    feature = "gamepad",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::runtime::registries::input::{GamepadButton, InputBinding, InputContext};

#[path = "gui_frame/pending_actions.rs"]
mod pending_actions;
#[path = "gui_frame/connected_open.rs"]
mod connected_open;
#[path = "gui_frame/graph_snapshot.rs"]
mod graph_snapshot;
#[path = "gui_frame/frame_persistence.rs"]
mod frame_persistence;
#[path = "gui_frame/workspace_layout.rs"]
mod workspace_layout;
#[path = "gui_frame/toolbar_dialog.rs"]
mod toolbar_dialog;
#[path = "gui_frame/keyboard_phase.rs"]
mod keyboard_phase;
#[path = "gui_frame/post_render_phase.rs"]
mod post_render_phase;

pub(crate) use keyboard_phase::{KeyboardPhaseArgs, handle_keyboard_phase};
pub(crate) use post_render_phase::{PostRenderPhaseArgs, run_post_render_phase};
pub(crate) use toolbar_dialog::{ToolbarDialogPhaseArgs, handle_toolbar_dialog_phase};

// Ownership map (Stage 4b gui_frame responsibility split):
// - `gui_frame.rs` remains the frame-phase facade and host for shared frame helpers.
// - `gui_frame/pending_actions.rs` owns post-render pending-action pipeline coordination.
// - Feature/domain helpers (frame snapshot, graph snapshot, workspace-layout handlers)
//   remain in this module and are invoked by the pending-actions coordinator.

const MAX_CONNECTED_SPLIT_PANES: usize = 4;
const MAX_CONNECTED_OPEN_NODES: usize = 12;

pub(crate) struct PreFrameIngestArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) app_state: &'a RunningAppState,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) thumbnail_capture_tx: &'a Sender<ThumbnailCaptureResult>,
    pub(crate) thumbnail_capture_rx: &'a Receiver<ThumbnailCaptureResult>,
    pub(crate) thumbnail_capture_in_flight: &'a mut HashSet<WebViewId>,
}

pub(crate) struct PreFrameIngestOutput {
    pub(crate) responsive_webviews: HashSet<WebViewId>,
}

pub(crate) fn ingest_pre_frame(
    args: PreFrameIngestArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
) -> PreFrameIngestOutput {
    let PreFrameIngestArgs {
        ctx,
        graph_app,
        app_state,
        window,
        favicon_textures,
        thumbnail_capture_tx,
        thumbnail_capture_rx,
        thumbnail_capture_in_flight,
    } = args;

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    {
        let focused_node = window
            .explicit_input_webview_id()
            .and_then(|webview_id| graph_app.get_node_for_webview(webview_id));
        let radial_menu_open = graph_app.workspace.show_radial_menu;
        for command in app_state.take_pending_gamepad_ui_commands() {
            let (binding, context) = match command {
                GamepadUiCommand::NavigateUp => (
                    InputBinding::Gamepad {
                        button: GamepadButton::DPadUp,
                        modifier: None,
                    },
                    if radial_menu_open {
                        InputContext::RadialMenuOpen
                    } else {
                        InputContext::GraphView
                    },
                ),
                GamepadUiCommand::NavigateDown => (
                    InputBinding::Gamepad {
                        button: GamepadButton::DPadDown,
                        modifier: None,
                    },
                    if radial_menu_open {
                        InputContext::RadialMenuOpen
                    } else {
                        InputContext::GraphView
                    },
                ),
                GamepadUiCommand::NavigateLeft => (
                    InputBinding::Gamepad {
                        button: GamepadButton::DPadLeft,
                        modifier: None,
                    },
                    if radial_menu_open {
                        InputContext::RadialMenuOpen
                    } else {
                        InputContext::GraphView
                    },
                ),
                GamepadUiCommand::NavigateRight => (
                    InputBinding::Gamepad {
                        button: GamepadButton::DPadRight,
                        modifier: None,
                    },
                    if radial_menu_open {
                        InputContext::RadialMenuOpen
                    } else {
                        InputContext::GraphView
                    },
                ),
                GamepadUiCommand::Confirm => (
                    InputBinding::Gamepad {
                        button: GamepadButton::LeftStickPress,
                        modifier: None,
                    },
                    InputContext::RadialMenuOpen,
                ),
                GamepadUiCommand::Cancel => (
                    InputBinding::Gamepad {
                        button: GamepadButton::East,
                        modifier: None,
                    },
                    InputContext::RadialMenuOpen,
                ),
                GamepadUiCommand::ToggleCommandPalette => (
                    InputBinding::Gamepad {
                        button: GamepadButton::Start,
                        modifier: None,
                    },
                    InputContext::GraphView,
                ),
                GamepadUiCommand::ToggleRadialMenu => (
                    InputBinding::Gamepad {
                        button: GamepadButton::South,
                        modifier: None,
                    },
                    InputContext::GraphView,
                ),
                GamepadUiCommand::NavigateBack => (
                    InputBinding::Gamepad {
                        button: GamepadButton::LeftBumper,
                        modifier: None,
                    },
                    InputContext::DetailView,
                ),
                GamepadUiCommand::NavigateForward => (
                    InputBinding::Gamepad {
                        button: GamepadButton::RightBumper,
                        modifier: None,
                    },
                    InputContext::DetailView,
                ),
            };

            let Some(action_id) = registries::phase2_resolve_typed_input_action_id(&binding, context)
            else {
                continue;
            };

            match action_id.as_str() {
                crate::shell::desktop::runtime::registries::input::ACTION_GRAPH_CYCLE_FOCUS_REGION => {
                    render::dispatch_action_id(
                        graph_app,
                        render::action_registry::ActionId::GraphCycleFocusRegion,
                        None,
                        focused_node,
                        focused_node,
                        None,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_GRAPH_COMMAND_PALETTE_OPEN => {
                    render::dispatch_action_id(
                        graph_app,
                        render::action_registry::ActionId::GraphCommandPalette,
                        None,
                        focused_node,
                        focused_node,
                        None,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_GRAPH_RADIAL_MENU_OPEN => {
                    render::dispatch_action_id(
                        graph_app,
                        render::action_registry::ActionId::GraphRadialMenu,
                        None,
                        focused_node,
                        focused_node,
                        None,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_TOOLBAR_NAV_BACK => {
                    let target = BrowserCommandTarget::ChromeProjection {
                        fallback_node: nav_targeting::chrome_projection_node(graph_app, window)
                            .or(focused_node),
                    };
                    graph_app.request_browser_command(target, BrowserCommand::Back);
                }
                crate::shell::desktop::runtime::registries::input::ACTION_TOOLBAR_NAV_FORWARD => {
                    let target = BrowserCommandTarget::ChromeProjection {
                        fallback_node: nav_targeting::chrome_projection_node(graph_app, window)
                            .or(focused_node),
                    };
                    graph_app.request_browser_command(target, BrowserCommand::Forward);
                }
                crate::shell::desktop::runtime::registries::input::ACTION_RADIAL_MENU_CATEGORY_PREVIOUS => {
                    render::radial_menu::queue_gamepad_input(
                        ctx,
                        render::radial_menu::RadialGamepadInput::NavigateLeft,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_RADIAL_MENU_CATEGORY_NEXT => {
                    render::radial_menu::queue_gamepad_input(
                        ctx,
                        render::radial_menu::RadialGamepadInput::NavigateRight,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_RADIAL_MENU_SELECTION_PREVIOUS => {
                    render::radial_menu::queue_gamepad_input(
                        ctx,
                        render::radial_menu::RadialGamepadInput::NavigateUp,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_RADIAL_MENU_SELECTION_NEXT => {
                    render::radial_menu::queue_gamepad_input(
                        ctx,
                        render::radial_menu::RadialGamepadInput::NavigateDown,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_RADIAL_MENU_CONFIRM => {
                    render::radial_menu::queue_gamepad_input(
                        ctx,
                        render::radial_menu::RadialGamepadInput::Confirm,
                    );
                }
                crate::shell::desktop::runtime::registries::input::ACTION_RADIAL_MENU_CANCEL => {
                    render::radial_menu::queue_gamepad_input(
                        ctx,
                        render::radial_menu::RadialGamepadInput::Cancel,
                    );
                }
                _ => {}
            }
        }
    }

    frame_intents.extend(thumbnail_pipeline::load_pending_thumbnail_results(
        graph_app,
        window,
        thumbnail_capture_rx,
        thumbnail_capture_in_flight,
    ));
    let (semantic_events, responsive_webviews) =
        semantic_event_pipeline::runtime_events_and_responsive_from_events(
            app_state.take_pending_graph_events(),
        );
    frame_intents.extend(semantic_events.into_iter().map(Into::into));
    frame_intents.extend(thumbnail_pipeline::load_pending_favicons(
        ctx,
        window,
        graph_app,
        favicon_textures,
    ));
    thumbnail_pipeline::request_pending_thumbnail_captures(
        graph_app,
        window,
        thumbnail_capture_tx,
        thumbnail_capture_in_flight,
    );

    PreFrameIngestOutput { responsive_webviews }
}

pub(crate) fn apply_intents_if_any(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    intents: &mut Vec<GraphIntent>,
) {
    if intents.is_empty() {
        return;
    }

    let mut undo_count = 0usize;
    let mut redo_count = 0usize;
    let mut apply_list = Vec::new();
    for intent in std::mem::take(intents) {
        match intent {
            GraphIntent::Undo => undo_count += 1,
            GraphIntent::Redo => redo_count += 1,
            other => apply_list.push(other),
        }
    }

    let layout_json = serde_json::to_string(tiles_tree).ok();
    if !apply_list.is_empty() {
        #[cfg(feature = "tracing")]
        let tracing_apply_started = Instant::now();

        #[cfg(feature = "tracing")]
        let _apply_span = tracing::trace_span!(
            "gui.apply_intents_if_any",
            apply_count = apply_list.len(),
            undo_count,
            redo_count,
        )
        .entered();

        #[cfg(feature = "diagnostics")]
        let apply_count = apply_list.len();
        #[cfg(feature = "diagnostics")]
        let apply_started = Instant::now();
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "graph_intents.apply",
            byte_len: apply_count,
        });
        graph_app.apply_reducer_intents_with_context(
            apply_list,
            ReducerDispatchContext {
                workspace_layout_before: layout_json.clone(),
                ..ReducerDispatchContext::default()
            },
        );
        #[cfg(feature = "diagnostics")]
        {
            let elapsed = apply_started.elapsed().as_micros() as u64;
            diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageReceived {
                channel_id: "graph_intents.apply",
                latency_us: elapsed,
            });
            diagnostics::emit_span_duration("gui_frame::apply_intents_if_any", elapsed);
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(
            target: "graphshell::perf",
            elapsed_us = tracing_apply_started.elapsed().as_micros() as u64,
            "gui.apply_intents_if_any.complete"
        );
    }

    if let Some(layout_json) = &layout_json {
        graph_app.mark_session_frame_layout_json(layout_json);
    }
    for _ in 0..undo_count {
        graph_app.apply_reducer_intents([GraphIntent::Undo]);
    }
    for _ in 0..redo_count {
        graph_app.apply_reducer_intents([GraphIntent::Redo]);
    }

    #[cfg(debug_assertions)]
    debug_assert!(
        intents.is_empty(),
        "intent buffer must be drained by apply_intents_if_any"
    );
}

pub(crate) struct LifecycleReconcilePhaseArgs<'a> {
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
}

fn history_preview_mode_active(graph_app: &GraphBrowserApp) -> bool {
    graph_app.history_health_summary().preview_mode_active
}

pub(crate) fn run_lifecycle_reconcile_and_apply(
    args: LifecycleReconcilePhaseArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let LifecycleReconcilePhaseArgs {
        graph_app,
        tiles_tree,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
    } = args;

    if history_preview_mode_active(graph_app) {
        frame_intents.clear();
        return;
    }

    if let Some(state) = app_state {
        while graph_app.take_pending_reload_all() {
            for window in state.windows().values() {
                window.set_needs_update();
                for (_, webview) in window.webviews() {
                    webview.reload();
                }
            }
        }
    }

    let reconcile_start_index = frame_intents.len();

    lifecycle_reconcile::reconcile_runtime(RuntimeReconcileArgs {
        graph_app,
        tiles_tree,
        window,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
        frame_intents,
    });

    #[cfg(debug_assertions)]
    {
        for intent in &frame_intents[reconcile_start_index..] {
            debug_assert!(
                !matches!(intent, GraphIntent::Undo | GraphIntent::Redo),
                "reconcile must not emit undo/redo intents"
            );
        }
    }

    apply_intents_if_any(graph_app, tiles_tree, frame_intents);

    // After intents are applied, ensure runtime viewers for Active nodes without tiles (prewarm).
    // Visible tile nodes are handled later in tile_render_pass.
    let mut prewarm_intents = lifecycle_reconcile::create_runtime_for_active_prewarm_nodes(
        ActivePrewarmArgs {
            graph_app,
            tiles_tree,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            responsive_webviews,
            webview_creation_backpressure,
        },
    );
    apply_intents_if_any(graph_app, tiles_tree, &mut prewarm_intents);

    #[cfg(debug_assertions)]
    debug_assert!(
        frame_intents.is_empty(),
        "frame intents must be empty after reconcile-and-apply phase"
    );
}

#[cfg(all(test, feature = "diagnostics"))]
mod tests {
    use super::*;
    use crate::app::GraphIntent;

    #[test]
    fn snapshot_restore_focus_reset_emits_ux_navigation_transition_channel() {
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let mut tree = Tree::new("graphshell_tiles", root, tiles);
        let mut tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>> =
            HashMap::new();
        let mut tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)> =
            HashMap::new();
        let mut webview_creation_backpressure: HashMap<NodeKey, WebviewCreationBackpressureState> =
            HashMap::new();
        let mut focused_node_hint = Some(NodeKey::new(9));
        let mut diagnostics = diagnostics::DiagnosticsState::new();

        graph_snapshot::reset_graph_workspace_after_snapshot_restore(
            &mut tree,
            &mut tile_rendering_contexts,
            &mut tile_favicon_textures,
            &mut webview_creation_backpressure,
            &mut focused_node_hint,
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when snapshot restore clears focus hint"
        );
    }

    #[test]
    fn history_preview_mode_active_tracks_preview_flag() {
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(!history_preview_mode_active(&app));

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        assert!(history_preview_mode_active(&app));

        app.apply_reducer_intents([GraphIntent::ExitHistoryTimelinePreview]);
        assert!(!history_preview_mode_active(&app));
    }
}

#[cfg(test)]
mod connected_open_tests {
    use super::*;
    use euclid::Point2D;

    #[test]
    fn connected_scope_depth_two_dedupes_shared_second_hop() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = app.add_node_and_sync("https://source.example".into(), Point2D::zero());
        let left = app.add_node_and_sync("https://left.example".into(), Point2D::new(10.0, 0.0));
        let right =
            app.add_node_and_sync("https://right.example".into(), Point2D::new(20.0, 0.0));
        let shared = app.add_node_and_sync(
            "https://shared.example".into(),
            Point2D::new(30.0, 0.0),
        );

        let _ = app.add_edge_and_sync(source, left, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(source, right, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(left, shared, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(right, shared, crate::model::graph::EdgeType::Hyperlink);

        let candidates = app.domain_graph().connected_candidates_with_depth(source, 2);

        assert!(candidates.contains(&(left, 1)));
        assert!(candidates.contains(&(right, 1)));
        assert!(candidates.contains(&(shared, 2)));
        assert_eq!(
            candidates
                .iter()
                .filter(|(key, depth)| *key == shared && *depth == 2)
                .count(),
            1,
            "shared second-hop candidate should be emitted once"
        );
    }

    #[test]
    fn neighbors_scope_reports_only_depth_one_neighbors() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = app.add_node_and_sync("https://source.example".into(), Point2D::zero());
        let neighbor = app.add_node_and_sync(
            "https://neighbor.example".into(),
            Point2D::new(10.0, 0.0),
        );
        let depth_two = app.add_node_and_sync(
            "https://depth-two.example".into(),
            Point2D::new(20.0, 0.0),
        );

        let _ = app.add_edge_and_sync(source, neighbor, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(neighbor, depth_two, crate::model::graph::EdgeType::Hyperlink);

        let candidates = app.domain_graph().connected_candidates_with_depth(source, 1);

        assert_eq!(candidates, vec![(neighbor, 1)]);
    }
}
