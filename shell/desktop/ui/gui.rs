/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use arboard::Clipboard;
use egui::pos2;
use egui_glow::EguiGlow;
use egui_tiles::{Tile, Tiles, Tree};
use egui_winit::EventResponse;
use euclid::{Length, Point2D};
use log::warn;
use servo::{
    DeviceIndependentPixel, LoadStatus, OffscreenRenderingContext, RenderingContext, WebViewId,
    WindowRenderingContext,
};
use url::Url;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use super::graph_search_flow::{self, GraphSearchFlowArgs};
use super::graph_search_ui::{self, GraphSearchUiArgs};
use super::gui_frame::{self, PreFrameIngestArgs, ToolbarDialogPhaseArgs};
use super::persistence_ops;
#[cfg(test)]
use super::thumbnail_pipeline;
use crate::shell::desktop::lifecycle::lifecycle_intents;
#[cfg(test)]
use crate::shell::desktop::lifecycle::semantic_event_pipeline;
use crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailCaptureResult;
use crate::shell::desktop::workbench::tile_compositor;
#[cfg(test)]
use crate::shell::desktop::workbench::tile_grouping;
#[cfg(test)]
use crate::shell::desktop::workbench::tile_invariants;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode, ToggleTileViewArgs};
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
use crate::shell::desktop::runtime::registries::{RegistryRuntime, knowledge};
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::lifecycle::webview_status_sync;
use crate::app::{
    ClipboardCopyKind, ClipboardCopyRequest, GraphBrowserApp, GraphIntent, GraphViewId, LifecycleCause,
    PendingTileOpenMode, SearchDisplayMode, ToastAnchorPreference,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_CLIPBOARD_COPY_FAILED;
use crate::shell::desktop::host::event_loop::AppEvent;
use crate::shell::desktop::host::headed_window;
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::services::search::fuzzy_match_node_keys;
use crate::shell::desktop::host::window::EmbedderWindow;
#[cfg(test)]
use crate::shell::desktop::host::window::GraphSemanticEvent;

struct ToolbarState {
    location: String,
    location_dirty: bool,
    location_submitted: bool,
    show_clear_data_confirm: bool,
    load_status: LoadStatus,
    status_text: Option<String>,
    can_go_back: bool,
    can_go_forward: bool,
}

struct GuiRuntimeState {
    graph_search_open: bool,
    graph_search_query: String,
    graph_search_filter_mode: bool,
    graph_search_matches: Vec<NodeKey>,
    graph_search_active_match_index: Option<usize>,
    focused_webview_hint: Option<WebViewId>,
    graph_surface_focused: bool,
    focus_ring_webview_id: Option<WebViewId>,
    focus_ring_started_at: Option<Instant>,
    focus_ring_duration: Duration,
    omnibar_search_session: Option<OmnibarSearchSession>,
    command_palette_toggle_requested: bool,
}

pub(crate) struct GuiUpdateInput<'a> {
    pub(crate) state: &'a RunningAppState,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) headed_window: &'a headed_window::HeadedWindow,
}

pub(crate) struct GuiUpdateOutput;

struct PreFramePhaseOutput {
    frame_intents: Vec<GraphIntent>,
    pending_open_child_webviews: Vec<WebViewId>,
    responsive_webviews: HashSet<WebViewId>,
}

/// The user interface of a headed servoshell. Currently this is implemented via
/// egui.
pub struct Gui {
    rendering_context: Rc<OffscreenRenderingContext>,
    window_rendering_context: Rc<WindowRenderingContext>,
    context: EguiGlow,
    /// Tile tree backing graph/detail pane layout.
    tiles_tree: Tree<TileKind>,
    toolbar_height: Length<f32, DeviceIndependentPixel>,

    toolbar_state: ToolbarState,
    /// Non-blocking toast notifications.
    toasts: egui_notify::Toasts,
    /// System clipboard handle.
    clipboard: Option<Clipboard>,

    /// Handle to the GPU texture of the favicon.
    ///
    /// These need to be cached across egui draw calls.
    favicon_textures: HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,

    /// Graph browser application state
    graph_app: GraphBrowserApp,

    /// Per-node offscreen rendering contexts for WebView tiles.
    tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>>,

    /// Per-node favicon textures for egui_tiles tab rendering.
    tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)>,

    /// Sender for asynchronous webview thumbnail capture results.
    thumbnail_capture_tx: Sender<ThumbnailCaptureResult>,

    /// Receiver for asynchronous webview thumbnail capture results.
    thumbnail_capture_rx: Receiver<ThumbnailCaptureResult>,

    /// WebViews with an in-flight thumbnail request.
    thumbnail_capture_in_flight: HashSet<WebViewId>,

    /// Runtime backpressure state for tile-driven webview creation retries.
    webview_creation_backpressure: HashMap<NodeKey, WebviewCreationBackpressureState>,

    /// Pending accessibility tree updates received from WebView/Servo that have
    /// not yet been injected into egui's accessibility tree.  Keyed by WebViewId
    /// so that a newer update from the same WebView supersedes the previous one.
    pending_webview_a11y_updates: HashMap<WebViewId, accesskit::TreeUpdate>,

    /// Cached reference to RunningAppState for webview creation
    state: Option<Rc<RunningAppState>>,

    /// Runtime UI state used by the frame coordinator and toolbar/search flows.
    runtime_state: GuiRuntimeState,

    #[cfg(feature = "diagnostics")]
    diagnostics_state: diagnostics::DiagnosticsState,

    /// Registry runtime for semantic services
    registry_runtime: RegistryRuntime,

    /// Tokio runtime for async background workers
    tokio_runtime: tokio::runtime::Runtime,

    /// Async worker supervision and intent queue
    control_panel: ControlPanel,
}

impl Drop for Gui {
    fn drop(&mut self) {
        if let Ok(layout_json) = serde_json::to_string(&self.tiles_tree) {
            self.graph_app.save_tile_layout_json(&layout_json);
        } else {
            warn!("Failed to serialize tile layout for persistence");
        }
        self.graph_app.take_snapshot();

        // Gracefully shutdown async workers
        self.tokio_runtime.block_on(async {
            self.control_panel.shutdown().await;
        });

        self.rendering_context
            .make_current()
            .expect("Could not make window RenderingContext current");
        self.context.destroy();
    }
}

fn restore_startup_session_workspace_if_available(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    if let Ok(bundle) = persistence_ops::load_named_workspace_bundle(
        graph_app,
        GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
    ) && let Ok((restored_tree, _)) =
        persistence_ops::restore_runtime_tree_from_workspace_bundle(graph_app, &bundle)
        && restored_tree.root().is_some()
    {
        if let Ok(runtime_layout_json) = serde_json::to_string(&restored_tree) {
            graph_app.mark_session_workspace_layout_json(&runtime_layout_json);
        }
        log::debug!("gui: restored startup session workspace from bundle");
        *tiles_tree = restored_tree;
        return true;
    }

    if let Some(layout_json) = graph_app.load_tile_layout_json()
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
        if restored_tree.root().is_some() {
            graph_app.mark_session_workspace_layout_json(&layout_json);
            log::debug!("gui: restored startup session workspace from legacy layout json");
            *tiles_tree = restored_tree;
            return true;
        }
    }
    false
}

impl Gui {
    fn toast_anchor(anchor: ToastAnchorPreference) -> egui_notify::Anchor {
        match anchor {
            ToastAnchorPreference::TopRight => egui_notify::Anchor::TopRight,
            ToastAnchorPreference::TopLeft => egui_notify::Anchor::TopLeft,
            ToastAnchorPreference::BottomRight => egui_notify::Anchor::BottomRight,
            ToastAnchorPreference::BottomLeft => egui_notify::Anchor::BottomLeft,
        }
    }

    fn open_mode_from_toolbar(mode: ToolbarOpenMode) -> TileOpenMode {
        match mode {
            ToolbarOpenMode::Tab => TileOpenMode::Tab,
            ToolbarOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
        }
    }

    fn open_mode_from_pending(mode: PendingTileOpenMode) -> TileOpenMode {
        match mode {
            PendingTileOpenMode::Tab => TileOpenMode::Tab,
            PendingTileOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
        }
    }

    fn default_open_mode_for_layout(_tiles_tree: &Tree<TileKind>) -> TileOpenMode {
        TileOpenMode::Tab
    }

    pub(crate) fn new(
        winit_window: &Window,
        event_loop: &ActiveEventLoop,
        event_loop_proxy: EventLoopProxy<AppEvent>,
        rendering_context: Rc<OffscreenRenderingContext>,
        window_rendering_context: Rc<WindowRenderingContext>,
        initial_url: Url,
        graph_data_dir: Option<PathBuf>,
        graph_snapshot_interval_secs: Option<u64>,
    ) -> Self {
        rendering_context
            .make_current()
            .expect("Could not make window RenderingContext current");
        let mut context = EguiGlow::new(
            event_loop,
            rendering_context.glow_gl_api(),
            None,
            None,
            false,
        );

        context
            .egui_winit
            .init_accesskit(event_loop, winit_window, event_loop_proxy);
        winit_window.set_visible(true);

        context.egui_ctx.options_mut(|options| {
            // Disable the builtin egui handlers for the Ctrl+Plus, Ctrl+Minus and Ctrl+0
            // shortcuts as they don't work well with servoshell's `device-pixel-ratio` CLI argument.
            options.zoom_with_keyboard = false;

            // On platforms where winit fails to obtain a system theme, fall back to a light theme
            // since it is the more common default.
            options.fallback_theme = egui::Theme::Light;
        });

        let initial_data_dir =
            graph_data_dir.unwrap_or_else(crate::services::persistence::GraphStore::default_data_dir);
        let mut graph_app = GraphBrowserApp::new_from_dir(initial_data_dir.clone());
        if let Some(snapshot_secs) = graph_snapshot_interval_secs
            && let Err(e) = graph_app.set_snapshot_interval_secs(snapshot_secs)
        {
            warn!("Failed to apply snapshot interval from startup preferences: {e}");
        }
        let mut tiles = Tiles::default();
        let graph_tile_id = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let mut tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);

        let _ = restore_startup_session_workspace_if_available(&mut graph_app, &mut tiles_tree);

        // Only create initial node if graph wasn't recovered from persistence
        if !graph_app.has_recovered_graph() {
            use euclid::default::Point2D;
            let _initial_node =
                graph_app.add_node_and_sync(initial_url.to_string(), Point2D::new(400.0, 300.0));
        }
        let membership_index =
            persistence_ops::build_membership_index_from_workspace_manifests(&graph_app);
        graph_app.init_membership_index(membership_index);
        let (workspace_recency, workspace_activation_seq) =
            persistence_ops::build_workspace_activation_recency_from_workspace_manifests(&graph_app);
        graph_app.init_workspace_activation_recency(workspace_recency, workspace_activation_seq);
        let (thumbnail_capture_tx, thumbnail_capture_rx) = channel();
        let initial_search_filter_mode =
            matches!(graph_app.workspace.search_display_mode, SearchDisplayMode::Filter);

        // Create tokio runtime for background workers
        let tokio_runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime for async workers");

        // Initialize ControlPanel and spawn workers inside runtime context
        let control_panel = {
            let _guard = tokio_runtime.enter();
            let mut panel = ControlPanel::new();
            panel.spawn_memory_monitor();
            panel.spawn_mod_loader();
            // Spawn sync worker if Verse mod is available
            panel.spawn_sync_worker();
            panel
        };
        graph_app.set_sync_command_tx(control_panel.sync_command_sender());

        Self {
            rendering_context,
            window_rendering_context,
            context,
            tiles_tree,
            toolbar_height: Default::default(),
            toolbar_state: ToolbarState {
                location: initial_url.to_string(),
                location_dirty: false,
                location_submitted: false,
                show_clear_data_confirm: false,
                load_status: LoadStatus::Complete,
                status_text: None,
                can_go_back: false,
                can_go_forward: false,
            },
            toasts: egui_notify::Toasts::default()
                .with_anchor(Self::toast_anchor(graph_app.workspace.toast_anchor_preference))
                .with_margin(egui::vec2(12.0, 12.0)),
            clipboard: Clipboard::new().ok(),
            favicon_textures: Default::default(),
            graph_app,
            tile_rendering_contexts: HashMap::new(),
            tile_favicon_textures: HashMap::new(),
            thumbnail_capture_tx,
            thumbnail_capture_rx,
            thumbnail_capture_in_flight: HashSet::new(),
            webview_creation_backpressure: HashMap::new(),
            pending_webview_a11y_updates: HashMap::new(),
            state: None,
            runtime_state: GuiRuntimeState {
                graph_search_open: false,
                graph_search_query: String::new(),
                graph_search_filter_mode: initial_search_filter_mode,
                graph_search_matches: Vec::new(),
                graph_search_active_match_index: None,
                focused_webview_hint: None,
                graph_surface_focused: false,
                focus_ring_webview_id: None,
                focus_ring_started_at: None,
                focus_ring_duration: Duration::from_millis(500),
                omnibar_search_session: None,
                command_palette_toggle_requested: false,
            },
            #[cfg(feature = "diagnostics")]
            diagnostics_state: diagnostics::DiagnosticsState::new(),
            registry_runtime: RegistryRuntime::new_with_mods(),
            tokio_runtime,
            control_panel,
        }
    }

    #[cfg(test)]
    fn parse_data_dir_input(raw: &str) -> Option<PathBuf> {
        persistence_ops::parse_data_dir_input(raw)
    }

    fn reset_runtime_webview_state(
        tiles_tree: &mut Tree<TileKind>,
        tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    ) {
        tile_runtime::reset_runtime_webview_state(
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
        );
    }

    pub(crate) fn is_graph_view(&self) -> bool {
        !self.has_active_node_pane()
    }

    /// Set the RunningAppState reference for webview creation
    pub(crate) fn set_state(&mut self, state: Rc<RunningAppState>) {
        self.state = Some(state);
    }

    pub(crate) fn surrender_focus(&self) {
        self.context.egui_ctx.memory_mut(|memory| {
            if let Some(focused) = memory.focused() {
                memory.surrender_focus(focused);
            }
        });
    }

    pub(crate) fn on_window_event(
        &mut self,
        winit_window: &Window,
        event: &WindowEvent,
    ) -> EventResponse {
        let mut response = self.context.on_window_event(winit_window, event);

        // When no WebView tile is active, consume user input events so they
        // never reach an inactive/hidden WebView.
        if !self.has_active_node_pane() {
            match event {
                WindowEvent::KeyboardInput { .. }
                | WindowEvent::ModifiersChanged(_)
                | WindowEvent::MouseInput { .. }
                | WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::Touch(_)
                | WindowEvent::PinchGesture { .. } => {
                    response.consumed = true;
                },
                _ => {},
            }
        }

        response
    }

    /// The height of the top toolbar of this user inteface ie the distance from the top of the
    /// window to the position of the `WebView`.
    pub(crate) fn toolbar_height(&self) -> Length<f32, DeviceIndependentPixel> {
        self.toolbar_height
    }

    pub(crate) fn webview_at_point(
        &self,
        point: Point2D<f32, DeviceIndependentPixel>,
    ) -> Option<(WebViewId, Point2D<f32, DeviceIndependentPixel>)> {
        let cursor = pos2(point.x, point.y);
        for tile_id in self.tiles_tree.active_tiles() {
            let Some(Tile::Pane(TileKind::Node(state))) = self.tiles_tree.tiles.get(tile_id)
            else {
                continue;
            };
            let Some(rect) = self.tiles_tree.tiles.rect(tile_id) else {
                continue;
            };
            if !rect.contains(cursor) {
                continue;
            }
            let Some(webview_id) = self.graph_app.get_webview_for_node(state.node) else {
                continue;
            };
            let local = Point2D::new(point.x - rect.min.x, point.y - rect.min.y);
            return Some((webview_id, local));
        }
        None
    }

    pub(crate) fn graph_at_point(&self, point: Point2D<f32, DeviceIndependentPixel>) -> bool {
        let cursor = pos2(point.x, point.y);
        self.tiles_tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                self.tiles_tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Graph(_)))
            ) && self
                .tiles_tree
                .tiles
                .rect(tile_id)
                .is_some_and(|rect| rect.contains(cursor))
        })
    }

    pub(crate) fn focused_webview_id(&self) -> Option<WebViewId> {
        if self.runtime_state.graph_surface_focused {
            return None;
        }
        tile_compositor::focused_webview_id_for_node_panes(
            &self.tiles_tree,
            &self.graph_app,
            self.runtime_state.focused_webview_hint,
        )
    }

    pub(crate) fn focused_tile_webview_id(&self) -> Option<WebViewId> {
        self.focused_webview_id()
    }

    #[allow(dead_code)]
    pub(crate) fn active_tile_webview_id(&self) -> Option<WebViewId> {
        tile_compositor::focused_webview_id_for_node_panes(
            &self.tiles_tree,
            &self.graph_app,
            None,
        )
    }

    pub(crate) fn set_focused_webview_id(&mut self, webview_id: WebViewId) {
        self.runtime_state.focused_webview_hint = Some(webview_id);
        self.runtime_state.graph_surface_focused = false;
    }

    pub(crate) fn focus_graph_surface(&mut self) {
        self.runtime_state.focused_webview_hint = None;
        self.runtime_state.graph_surface_focused = true;
        self.graph_app.workspace.focused_view =
            tile_view_ops::active_graph_view_id(&self.tiles_tree);
    }

    pub(crate) fn location_has_focus(&self) -> bool {
        self.context.egui_ctx.memory(|m| {
            m.focused()
                .is_some_and(|focused| focused == egui::Id::new("location_input"))
        })
    }

    pub(crate) fn request_location_submit(&mut self) {
        self.toolbar_state.location_submitted = true;
    }

    pub(crate) fn request_command_palette_toggle(&mut self) {
        self.runtime_state.command_palette_toggle_requested = true;
    }

    pub(crate) fn egui_wants_keyboard_input(&self) -> bool {
        self.context.egui_ctx.wants_keyboard_input()
    }

    pub(crate) fn egui_wants_pointer_input(&self) -> bool {
        self.context.egui_ctx.wants_pointer_input()
    }

    pub(crate) fn pointer_hover_position(&self) -> Option<Point2D<f32, DeviceIndependentPixel>> {
        self.context
            .egui_ctx
            .input(|i| i.pointer.hover_pos())
            .map(|p| Point2D::new(p.x, p.y))
    }

    pub(crate) fn ui_overlay_active(&self) -> bool {
        self.graph_app.workspace.show_command_palette
            || self.graph_app.workspace.show_help_panel
            || self.graph_app.workspace.show_physics_panel
            || self.toolbar_state.show_clear_data_confirm
    }

    /// Update the user interface, but do not paint the updated state.
    pub(crate) fn update(
        &mut self,
        state: &RunningAppState,
        window: &EmbedderWindow,
        headed_window: &headed_window::HeadedWindow,
    ) {
        let _ = self.run_update(GuiUpdateInput {
            state,
            window,
            headed_window,
        });
    }

    fn run_update(&mut self, input: GuiUpdateInput<'_>) -> GuiUpdateOutput {
        let GuiUpdateInput {
            state,
            window,
            headed_window,
        } = input;
        // Note: We need Rc<RunningAppState> for webview creation, but this method
        // is called from trait methods that only provide &RunningAppState.
        // The caller should have Rc available at the call site.
        self.rendering_context
            .make_current()
            .expect("Could not make RenderingContext current");
        self.ensure_tiles_tree_root();
        debug_assert!(
            self.tiles_tree.root().is_some(),
            "tile tree root must exist before rendering"
        );
        let Self {
            rendering_context,
            window_rendering_context,
            context,
            tiles_tree,
            toolbar_height,
            toolbar_state,
            toasts,
            clipboard,
            favicon_textures,
            graph_app,
            tile_rendering_contexts,
            tile_favicon_textures,
            thumbnail_capture_tx,
            thumbnail_capture_rx,
            thumbnail_capture_in_flight,
            webview_creation_backpressure,
            pending_webview_a11y_updates,
            state: app_state,
            runtime_state,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            registry_runtime,
            control_panel,
            ..
        } = self;
        let GuiRuntimeState {
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            focused_webview_hint,
            graph_surface_focused,
            focus_ring_webview_id,
            focus_ring_started_at,
            focus_ring_duration,
            omnibar_search_session,
            command_palette_toggle_requested,
        } = runtime_state;

        let winit_window = headed_window.winit_window();
        *toasts = std::mem::take(toasts)
            .with_anchor(Self::toast_anchor(graph_app.workspace.toast_anchor_preference))
            .with_margin(egui::vec2(12.0, 12.0));
        context.run(winit_window, |ctx| {
            graph_app.tick_frame();

            // Inject any pending WebView accessibility tree updates into egui's
            // accessibility tree.  Each Servo node is registered using an
            // identity-preserving egui::Id derived from its accesskit::NodeId so
            // that child references inside Node objects remain valid.
            Self::inject_webview_a11y_updates(ctx, pending_webview_a11y_updates);

            #[cfg(feature = "diagnostics")]
            {
                let toggle_diagnostics = ctx.input(|i| {
                    i.key_pressed(egui::Key::F12)
                        || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::D))
                });
                if toggle_diagnostics {
                    Self::open_or_focus_diagnostics_tool_pane(tiles_tree);
                }
            }
            let pre_frame = Self::run_pre_frame_phase(
                ctx,
                graph_app,
                state,
                window,
                favicon_textures,
                thumbnail_capture_tx,
                thumbnail_capture_rx,
                thumbnail_capture_in_flight,
                command_palette_toggle_requested,
            );
            let mut frame_intents = pre_frame.frame_intents;

            // Drain async worker intents from Control Panel
            frame_intents.extend(control_panel.drain_pending());

            let mut open_node_tile_after_intents: Option<TileOpenMode> = None;

            let mut graph_search_output = Self::run_graph_search_phase(
                ctx,
                graph_app,
                tiles_tree,
                graph_search_open,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
                toolbar_state,
                &mut frame_intents,
            );

            gui_frame::handle_keyboard_phase(
                gui_frame::KeyboardPhaseArgs {
                    ctx,
                    graph_app,
                    window,
                    tiles_tree,
                    tile_rendering_contexts,
                    tile_favicon_textures,
                    favicon_textures,
                    app_state,
                    rendering_context,
                    window_rendering_context,
                    responsive_webviews: &pre_frame.responsive_webviews,
                    webview_creation_backpressure,
                    suppress_toggle_view: graph_search_output.suppress_toggle_view,
                },
                &mut frame_intents,
                |tiles_tree,
                 graph_app,
                 window,
                 app_state,
                 rendering_context,
                 window_rendering_context,
                 tile_rendering_contexts,
                 responsive_webviews,
                 webview_creation_backpressure,
                 frame_intents| {
                    Self::toggle_tile_view(
                        tiles_tree,
                        graph_app,
                        window,
                        app_state,
                        rendering_context,
                        window_rendering_context,
                        tile_rendering_contexts,
                        responsive_webviews,
                        webview_creation_backpressure,
                        frame_intents,
                    );
                },
                |tiles_tree, tile_rendering_contexts, tile_favicon_textures, favicon_textures| {
                    Self::reset_runtime_webview_state(
                        tiles_tree,
                        tile_rendering_contexts,
                        tile_favicon_textures,
                        favicon_textures,
                    );
                },
            );

            let (toolbar_visible, is_graph_view) = Self::run_toolbar_phase(
                ctx,
                winit_window,
                state,
                graph_app,
                #[cfg(feature = "diagnostics")]
                diagnostics_state,
                window,
                tiles_tree,
                *focused_webview_hint,
                *graph_surface_focused,
                toolbar_state,
                graph_search_output.focus_location_field_for_search,
                omnibar_search_session,
                toasts,
                tile_rendering_contexts,
                tile_favicon_textures,
                favicon_textures,
                app_state,
                rendering_context,
                window_rendering_context,
                &pre_frame.responsive_webviews,
                webview_creation_backpressure,
                &mut frame_intents,
                &mut open_node_tile_after_intents,
            );

            if toolbar_visible && *graph_search_open && is_graph_view {
                graph_search_ui::render_graph_search_window(
                    GraphSearchUiArgs {
                        ctx,
                        graph_app,
                        graph_search_query,
                        graph_search_filter_mode,
                        graph_search_matches,
                        graph_search_active_match_index,
                        focus_graph_search_field: &mut graph_search_output.focus_graph_search_field,
                    },
                    |graph_app, query, matches, active_index| {
                        refresh_graph_search_matches(graph_app, query, matches, active_index);
                    },
                );
            }

            // Workbench-layer pane intents (P6) mutate tile state directly and should
            // not flow through GraphBrowserApp's semantic reducer.
            Self::handle_tool_pane_intents(tiles_tree, &mut frame_intents);

            // Phase 1: apply semantic/UI intents before lifecycle reconciliation.
            gui_frame::apply_intents_if_any(graph_app, tiles_tree, &mut frame_intents);
            // Bridge legacy panel flags to pane-hosted tool surfaces where mappings exist.
            // This preserves current panel behavior while letting pane-hosted architecture
            // become the visible runtime path.
            Self::bridge_legacy_panel_flags_to_tool_panes(graph_app, tiles_tree);
            Self::handle_pending_open_node_after_intents(
                graph_app,
                tiles_tree,
                &mut open_node_tile_after_intents,
                &mut frame_intents,
            );
            gui_frame::open_pending_child_webviews_for_tiles(
                graph_app,
                pre_frame.pending_open_child_webviews,
                |node_key| {
                    tile_view_ops::open_or_focus_node_pane_with_mode(
                        tiles_tree,
                        node_key,
                        Self::default_open_mode_for_layout(tiles_tree),
                    );
                },
            );
            gui_frame::run_lifecycle_reconcile_and_apply(
                gui_frame::LifecycleReconcilePhaseArgs {
                    graph_app,
                    tiles_tree,
                    window,
                    app_state,
                    rendering_context,
                    window_rendering_context,
                    tile_rendering_contexts,
                    tile_favicon_textures,
                    favicon_textures,
                    responsive_webviews: &pre_frame.responsive_webviews,
                    webview_creation_backpressure,
                },
                &mut frame_intents,
            );

            // Phase 2: Reconcile semantic index (UDC codes)
            knowledge::reconcile_semantics(graph_app, &registry_runtime.knowledge);

            gui_frame::run_post_render_phase(
                gui_frame::PostRenderPhaseArgs {
                    ctx,
                    graph_app,
                    window,
                    headed_window,
                    tiles_tree,
                    tile_rendering_contexts,
                    tile_favicon_textures,
                    favicon_textures,
                    toolbar_height,
                    graph_search_matches,
                    graph_search_active_match_index: *graph_search_active_match_index,
                    graph_search_filter_mode: *graph_search_filter_mode,
                    search_query_active: !graph_search_query.trim().is_empty(),
                    app_state,
                    rendering_context,
                    window_rendering_context,
                    responsive_webviews: &pre_frame.responsive_webviews,
                    webview_creation_backpressure,
                    focused_webview_hint,
                    graph_surface_focused: *graph_surface_focused,
                    focus_ring_webview_id,
                    focus_ring_started_at,
                    focus_ring_duration: *focus_ring_duration,
                    toasts,
                    control_panel,
                    #[cfg(feature = "diagnostics")]
                    diagnostics_state,
                },
                |matches, active_index| active_graph_search_match(matches, active_index),
            );
            Self::handle_pending_clipboard_copy_requests(graph_app, clipboard, toasts);
            toasts.show(ctx);
        });

        GuiUpdateOutput
    }

    #[allow(clippy::too_many_arguments)]
    fn run_pre_frame_phase(
        ctx: &egui::Context,
        graph_app: &mut GraphBrowserApp,
        state: &RunningAppState,
        window: &EmbedderWindow,
        favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
        thumbnail_capture_tx: &Sender<ThumbnailCaptureResult>,
        thumbnail_capture_rx: &Receiver<ThumbnailCaptureResult>,
        thumbnail_capture_in_flight: &mut HashSet<WebViewId>,
        command_palette_toggle_requested: &mut bool,
    ) -> PreFramePhaseOutput {
        let mut frame_intents = Vec::new();
        if *command_palette_toggle_requested {
            *command_palette_toggle_requested = false;
            frame_intents.push(GraphIntent::ToggleCommandPalette);
        }

        let pre_frame = gui_frame::ingest_pre_frame(
            PreFrameIngestArgs {
                ctx,
                graph_app,
                app_state: state,
                window,
                favicon_textures,
                thumbnail_capture_tx,
                thumbnail_capture_rx,
                thumbnail_capture_in_flight,
            },
            &mut frame_intents,
        );
        PreFramePhaseOutput {
            frame_intents,
            pending_open_child_webviews: pre_frame.pending_open_child_webviews,
            responsive_webviews: pre_frame.responsive_webviews,
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn run_graph_search_phase(
        ctx: &egui::Context,
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &Tree<TileKind>,
        graph_search_open: &mut bool,
        graph_search_query: &mut String,
        graph_search_filter_mode: &mut bool,
        graph_search_matches: &mut Vec<NodeKey>,
        graph_search_active_match_index: &mut Option<usize>,
        toolbar_state: &mut ToolbarState,
        frame_intents: &mut Vec<GraphIntent>,
    ) -> graph_search_flow::GraphSearchFlowOutput {
        let graph_search_available = Self::active_node_pane_node(tiles_tree).is_none();
        graph_app.workspace.search_display_mode = if *graph_search_filter_mode {
            SearchDisplayMode::Filter
        } else {
            SearchDisplayMode::Highlight
        };
        graph_search_flow::handle_graph_search_flow(
            GraphSearchFlowArgs {
                ctx,
                graph_app,
                graph_search_open,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
                location: &mut toolbar_state.location,
                location_dirty: &mut toolbar_state.location_dirty,
                frame_intents,
                graph_search_available,
            },
            |graph_app, query, matches, active_index| {
                refresh_graph_search_matches(graph_app, query, matches, active_index);
            },
            |matches, active_index, delta| {
                step_graph_search_active_match(matches, active_index, delta);
            },
            |matches, active_index| active_graph_search_match(matches, active_index),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_toolbar_phase(
        ctx: &egui::Context,
        winit_window: &Window,
        state: &RunningAppState,
        graph_app: &mut GraphBrowserApp,
        #[cfg(feature = "diagnostics")]
        diagnostics_state: &mut diagnostics::DiagnosticsState,
        window: &EmbedderWindow,
        tiles_tree: &mut Tree<TileKind>,
        focused_webview_hint: Option<WebViewId>,
        graph_surface_focused: bool,
        toolbar_state: &mut ToolbarState,
        focus_location_field_for_search: bool,
        omnibar_search_session: &mut Option<OmnibarSearchSession>,
        toasts: &mut egui_notify::Toasts,
        tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
        app_state: &Option<Rc<RunningAppState>>,
        rendering_context: &Rc<OffscreenRenderingContext>,
        window_rendering_context: &Rc<WindowRenderingContext>,
        responsive_webviews: &HashSet<WebViewId>,
        webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
        frame_intents: &mut Vec<GraphIntent>,
        open_node_tile_after_intents: &mut Option<TileOpenMode>,
    ) -> (bool, bool) {
        let toolbar_dialog_phase = gui_frame::handle_toolbar_dialog_phase(
            ToolbarDialogPhaseArgs {
                ctx,
                winit_window,
                state,
                graph_app,
                window,
                tiles_tree,
                focused_webview_hint,
                graph_surface_focused,
                can_go_back: toolbar_state.can_go_back,
                can_go_forward: toolbar_state.can_go_forward,
                location: &mut toolbar_state.location,
                location_dirty: &mut toolbar_state.location_dirty,
                location_submitted: &mut toolbar_state.location_submitted,
                focus_location_field_for_search,
                show_clear_data_confirm: &mut toolbar_state.show_clear_data_confirm,
                omnibar_search_session,
                toasts,
                tile_rendering_contexts,
                tile_favicon_textures,
                favicon_textures,
                #[cfg(feature = "diagnostics")]
                diagnostics_state,
            },
            frame_intents,
        );
        let toolbar_output = toolbar_dialog_phase.toolbar_output;
        let is_graph_view = toolbar_dialog_phase.is_graph_view;
        if toolbar_output.toggle_tile_view_requested {
            Self::toggle_tile_view(
                tiles_tree,
                graph_app,
                window,
                app_state,
                rendering_context,
                window_rendering_context,
                tile_rendering_contexts,
                responsive_webviews,
                webview_creation_backpressure,
                frame_intents,
            );
        }
        if let Some(open_mode) = toolbar_output.open_selected_mode_after_submit {
            *open_node_tile_after_intents = Some(Self::open_mode_from_toolbar(open_mode));
        }

        (toolbar_output.toolbar_visible, is_graph_view)
    }

    fn handle_pending_clipboard_copy_requests(
        graph_app: &mut GraphBrowserApp,
        clipboard: &mut Option<Clipboard>,
        toasts: &mut egui_notify::Toasts,
    ) {
        while let Some(ClipboardCopyRequest { key, kind }) = graph_app.take_pending_clipboard_copy()
        {
            let Some(node) = graph_app.workspace.graph.get_node(key) else {
                toasts.error("Copy failed: node no longer exists");
                continue;
            };
            let value = match kind {
                ClipboardCopyKind::Url => node.url.clone(),
                ClipboardCopyKind::Title => {
                    if node.title.is_empty() {
                        node.url.clone()
                    } else {
                        node.title.clone()
                    }
                },
            };
            if value.trim().is_empty() {
                toasts.warning("Nothing to copy");
                continue;
            }
            if clipboard.is_none() {
                *clipboard = Clipboard::new().ok();
            }
            let Some(cb) = clipboard.as_mut() else {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_UI_CLIPBOARD_COPY_FAILED,
                    byte_len: "clipboard unavailable".len(),
                });
                toasts.error("Clipboard unavailable");
                continue;
            };
            match cb.set_text(value) {
                Ok(()) => match kind {
                    ClipboardCopyKind::Url => {
                        toasts.success("Copied URL");
                    },
                    ClipboardCopyKind::Title => {
                        toasts.success("Copied title");
                    },
                },
                Err(e) => {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UI_CLIPBOARD_COPY_FAILED,
                        byte_len: e.to_string().len(),
                    });
                    toasts.error(format!("Copy failed: {e}"));
                },
            }
        }
    }

    fn ensure_tiles_tree_root(&mut self) {
        if self.tiles_tree.root().is_none() {
            let graph_tile_id = self
                .tiles_tree
                .tiles
                .insert_pane(TileKind::Graph(GraphViewId::default()));
            self.tiles_tree.root = Some(graph_tile_id);
        }
    }

    fn toggle_tile_view(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &mut GraphBrowserApp,
        window: &EmbedderWindow,
        app_state: &Option<Rc<RunningAppState>>,
        base_rendering_context: &Rc<OffscreenRenderingContext>,
        window_rendering_context: &Rc<WindowRenderingContext>,
        tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        responsive_webviews: &HashSet<WebViewId>,
        webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
        lifecycle_intents: &mut Vec<GraphIntent>,
    ) {
        tile_view_ops::toggle_tile_view(ToggleTileViewArgs {
            tiles_tree,
            graph_app,
            window,
            app_state,
            base_rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            responsive_webviews,
            webview_creation_backpressure,
            lifecycle_intents,
        });
    }

    #[cfg(feature = "diagnostics")]
    fn open_or_focus_tool_pane(
        tiles_tree: &mut Tree<TileKind>,
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    ) {
        if tiles_tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Tool(tool)) if tool == &kind)
        }) {
            return;
        }

        let tool_tile_id = tiles_tree.tiles.insert_pane(TileKind::Tool(kind.clone()));
        let Some(root_id) = tiles_tree.root() else {
            tiles_tree.root = Some(tool_tile_id);
            return;
        };

        if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) =
            tiles_tree.tiles.get_mut(root_id)
        {
            tabs.add_child(tool_tile_id);
            tabs.set_active(tool_tile_id);
            return;
        }

        let tabs_root = tiles_tree.tiles.insert_tab_tile(vec![root_id, tool_tile_id]);
        tiles_tree.root = Some(tabs_root);
        let _ = tiles_tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Tool(tool)) if tool == &kind)
        });
    }

    #[cfg(not(feature = "diagnostics"))]
    fn open_or_focus_tool_pane(
        _tiles_tree: &mut Tree<TileKind>,
        _kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    ) {
    }

    #[cfg(feature = "diagnostics")]
    fn open_or_focus_diagnostics_tool_pane(tiles_tree: &mut Tree<TileKind>) {
        use crate::shell::desktop::workbench::pane_model::ToolPaneState;
        Self::open_or_focus_tool_pane(tiles_tree, ToolPaneState::Diagnostics);
    }

    #[cfg(not(feature = "diagnostics"))]
    fn open_or_focus_diagnostics_tool_pane(_tiles_tree: &mut Tree<TileKind>) {}

    /// Intercept workbench-authority intents before they reach `apply_intents()`.
    ///
    /// ## Two-authority model
    ///
    /// The architecture has two distinct mutation authorities:
    ///
    /// - **Graph Reducer** (`apply_intents` in `app.rs`): authoritative for the graph
    ///   data model, node/edge lifecycle, WAL journal, and traversal history.
    ///   Always synchronous, always logged, always testable.
    ///
    /// - **Workbench Authority** (this function + `tile_view_ops.rs`): authoritative
    ///   for tile-tree shape mutations (`egui_tiles` splits, tabs, pane open/close/
    ///   focus). The tile tree is a layout construct — not graph state — and must
    ///   not flow through the graph reducer or the WAL.
    ///
    /// Intents tagged as workbench-authority (`OpenToolPane`, `SplitPane`,
    /// `SetPaneView`, `OpenNodeInPane`) must be drained here, before `apply_intents`
    /// is called. Any that leak through will produce a `log::warn!` in the reducer.
    fn handle_tool_pane_intents(
        tiles_tree: &mut Tree<TileKind>,
        frame_intents: &mut Vec<GraphIntent>,
    ) {
        let mut remaining = Vec::with_capacity(frame_intents.len());
        for intent in frame_intents.drain(..) {
            match intent {
                GraphIntent::OpenToolPane { kind } => {
                    Self::open_or_focus_tool_pane(tiles_tree, kind);
                }
                GraphIntent::OpenNodeInPane { node, pane } => {
                    log::debug!(
                        "workbench intent OpenNodeInPane ignored pane target {}; opening node pane directly",
                        pane
                    );
                    tile_view_ops::open_or_focus_node_pane(tiles_tree, node);
                }
                GraphIntent::SetPaneView { pane, view } => {
                    log::debug!(
                        "workbench intent SetPaneView ignored pane target {}; applying view payload",
                        pane
                    );
                    match view {
                        crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(kind) => {
                            Self::open_or_focus_tool_pane(tiles_tree, kind);
                        }
                        crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state) => {
                            tile_view_ops::open_or_focus_node_pane(tiles_tree, state.node);
                        }
                        crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref) => {
                            tile_view_ops::open_or_focus_graph_pane(
                                tiles_tree,
                                graph_ref.graph_view_id,
                            );
                        }
                    }
                }
                GraphIntent::SplitPane {
                    source_pane,
                    direction,
                } => {
                    if matches!(
                        direction,
                        crate::shell::desktop::workbench::pane_model::SplitDirection::Vertical
                    ) {
                        log::debug!(
                            "workbench intent SplitPane({source_pane}, {:?}) currently maps to horizontal split in tile_view_ops",
                            direction
                        );
                    }
                    let new_view_id = GraphViewId::new();
                    tile_view_ops::open_or_focus_graph_pane_with_mode(
                        tiles_tree,
                        new_view_id,
                        TileOpenMode::SplitHorizontal,
                    );
                }
                other => remaining.push(other),
            }
        }
        *frame_intents = remaining;
    }

    fn bridge_legacy_panel_flags_to_tool_panes(
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &mut Tree<TileKind>,
    ) {
        use crate::shell::desktop::workbench::pane_model::ToolPaneState;

        if graph_app.workspace.show_history_manager {
            Self::open_or_focus_tool_pane(tiles_tree, ToolPaneState::HistoryManager);
            graph_app.workspace.show_history_manager = false;
        }

        // Persistence/sync/settings URLs still route through legacy booleans today.
        // Bridge those requests to the unified Settings tool pane while compatibility
        // panels continue to exist during migration.
        if graph_app.workspace.show_persistence_panel || graph_app.workspace.show_sync_panel {
            Self::open_or_focus_tool_pane(tiles_tree, ToolPaneState::Settings);
            graph_app.workspace.show_persistence_panel = false;
            graph_app.workspace.show_sync_panel = false;
        }
    }

    fn handle_pending_open_node_after_intents(
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &mut Tree<TileKind>,
        open_node_tile_after_intents: &mut Option<TileOpenMode>,
        frame_intents: &mut Vec<GraphIntent>,
    ) {
        if let Some(open_request) = graph_app.take_pending_open_node_request() {
            log::debug!("gui: handle_pending_open_node_after_intents taking request for {:?}", open_request.key);
            *open_node_tile_after_intents = Some(Self::open_mode_from_pending(open_request.mode));
            graph_app.select_node(open_request.key, false);
        }

        if let Some(open_mode) = *open_node_tile_after_intents
            && let Some(node_key) = graph_app.get_single_selected_node()
        {
            if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
                graph_app.capture_undo_checkpoint(Some(layout_json));
            }
            let anchor_before_open = if open_mode == TileOpenMode::Tab {
                gui_frame::active_node_pane_node(tiles_tree)
            } else {
                None
            };
            let node_already_in_workspace = tiles_tree.tiles.iter().any(|(_, tile)| {
                matches!(
                    tile,
                    Tile::Pane(TileKind::Node(state)) if state.node == node_key
                )
            });
            log::debug!("gui: calling open_or_focus_node_pane_with_mode for {:?} mode {:?}", node_key, open_mode);
            tile_view_ops::open_or_focus_node_pane_with_mode(tiles_tree, node_key, open_mode);
            if open_mode == TileOpenMode::Tab
                && !node_already_in_workspace
                && let Some(anchor) = anchor_before_open
                && anchor != node_key
            {
                frame_intents.push(GraphIntent::CreateUserGroupedEdge {
                    from: anchor,
                    to: node_key,
                });
            }
            frame_intents.push(lifecycle_intents::promote_node_to_active(
                node_key,
                LifecycleCause::UserSelect,
            ));
        }
    }

    fn has_active_node_pane(&self) -> bool {
        self.tiles_tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                self.tiles_tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Node(_)))
            )
        })
    }

    fn active_node_pane_node(tiles_tree: &Tree<TileKind>) -> Option<crate::graph::NodeKey> {
        tiles_tree.active_tiles().into_iter().find_map(|tile_id| {
            match tiles_tree.tiles.get(tile_id) {
                Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
                _ => None,
            }
        })
    }

    /// Paint the GUI, as of the last update.
    pub(crate) fn paint(&mut self, window: &Window) {
        self.rendering_context
            .make_current()
            .expect("Could not make RenderingContext current");
        self.rendering_context
            .parent_context()
            .prepare_for_rendering();
        self.context.paint(window);
        self.rendering_context.parent_context().present();
    }

    /// Updates the location field from the given [`RunningAppState`], unless the user has started
    /// editing it without clicking Go, returning true iff it has changed (needing an egui update).
    fn update_location_in_toolbar(&mut self, window: &EmbedderWindow) -> bool {
        if self.toolbar_state.location.trim_start().starts_with('@') {
            // Preserve active omnibar node-search query text while cycling matches.
            return false;
        }
        let has_node_panes = tile_runtime::has_any_node_panes(&self.tiles_tree);
        let selected_node_url = self.graph_app.get_single_selected_node().and_then(|key| {
            self.graph_app
                .workspace
                .graph
                .get_node(key)
                .map(|node| node.url.clone())
        });
        let focused_webview_id = self.focused_webview_id();
        webview_status_sync::update_location_in_toolbar(
            self.toolbar_state.location_dirty,
            &mut self.toolbar_state.location,
            has_node_panes,
            selected_node_url,
            focused_webview_id,
            window,
        )
    }

    fn update_load_status(&mut self, window: &EmbedderWindow) -> bool {
        let focused_webview_id = self.focused_webview_id();
        webview_status_sync::update_load_status(
            &mut self.toolbar_state.load_status,
            &mut self.toolbar_state.location_dirty,
            focused_webview_id,
            window,
        )
    }

    fn update_status_text(&mut self, window: &EmbedderWindow) -> bool {
        let focused_webview_id = self.focused_webview_id();
        webview_status_sync::update_status_text(
            &mut self.toolbar_state.status_text,
            focused_webview_id,
            window,
        )
    }

    fn update_can_go_back_and_forward(&mut self, window: &EmbedderWindow) -> bool {
        let focused_webview_id = self.focused_webview_id();
        webview_status_sync::update_can_go_back_and_forward(
            &mut self.toolbar_state.can_go_back,
            &mut self.toolbar_state.can_go_forward,
            focused_webview_id,
            window,
        )
    }

    /// Updates all fields taken from the given [`EmbedderWindow`], such as the location field.
    /// Returns true iff the egui needs an update.
    pub(crate) fn update_webview_data(&mut self, window: &EmbedderWindow) -> bool {
        // Note: We must use the "bitwise OR" (|) operator here instead of "logical OR" (||)
        //       because logical OR would short-circuit if any of the functions return true.
        //       We want to ensure that all functions are called. The "bitwise OR" operator
        //       does not short-circuit.
        self.update_load_status(window)
            | self.update_location_in_toolbar(window)
            | self.update_status_text(window)
            | self.update_can_go_back_and_forward(window)
    }

    /// Returns true if a redraw is required after handling the provided event.
    pub(crate) fn handle_accesskit_event(
        &mut self,
        event: &egui_winit::accesskit_winit::WindowEvent,
    ) -> bool {
        match event {
            egui_winit::accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.context.egui_ctx.enable_accesskit();
                true
            },
            egui_winit::accesskit_winit::WindowEvent::ActionRequested(req) => {
                self.context
                    .egui_winit
                    .on_accesskit_action_request(req.clone());
                true
            },
            egui_winit::accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.context.egui_ctx.disable_accesskit();
                false
            },
        }
    }

    pub(crate) fn set_zoom_factor(&self, factor: f32) {
        let clamped = if factor.is_finite() {
            factor.clamp(0.25, 4.0)
        } else {
            1.0
        };
        self.context.egui_ctx.set_zoom_factor(clamped);
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn diagnostics_state(&self) -> &diagnostics::DiagnosticsState {
        &self.diagnostics_state
    }

    pub(crate) fn notify_accessibility_tree_update(
        &mut self,
        webview_id: WebViewId,
        tree_update: accesskit::TreeUpdate,
    ) {
        // Store the most recent update per WebView; it will be injected into
        // egui's accessibility tree at the start of the next frame inside
        // the context.run() callback.
        self.pending_webview_a11y_updates.insert(webview_id, tree_update);
    }

    fn webview_accessibility_anchor_id(webview_id: WebViewId) -> egui::Id {
        egui::Id::new(("webview_accessibility_anchor", webview_id))
    }

    fn webview_accessibility_label(
        webview_id: WebViewId,
        tree_update: &accesskit::TreeUpdate,
    ) -> String {
        if let Some((_, focused_node)) = tree_update
            .nodes
            .iter()
            .find(|(node_id, _)| *node_id == tree_update.focus)
            && let Some(label) = focused_node.label()
            && !label.trim().is_empty()
        {
            return format!("Embedded web content: {label}");
        }

        if let Some((_, first_labeled)) = tree_update
            .nodes
            .iter()
            .find(|(_, node)| node.label().is_some_and(|label| !label.trim().is_empty()))
            && let Some(label) = first_labeled.label()
        {
            return format!("Embedded web content: {label}");
        }

        format!(
            "Embedded web content (webview {:?}, {} accessibility node update(s))",
            webview_id,
            tree_update.nodes.len()
        )
    }

    /// Inject pending WebView accessibility tree updates into egui's
    /// accessibility tree.
    ///
    /// For each node in a Servo-provided `accesskit::TreeUpdate`, an egui `Id`
    /// is derived by treating the Servo `NodeId` (a `u64`) as a high-entropy
    /// value.  This preserves the identity mapping
    /// `egui_id.accesskit_id() == servo_node_id`, which means child-reference
    /// arrays inside the injected `accesskit::Node` objects remain valid once
    /// egui builds the final `TreeUpdate`.
    ///
    /// Nodes whose `NodeId` is zero or `u64::MAX` (egui's root sentinel) are
    /// skipped to avoid collisions with egui's own accessibility tree.
    fn inject_webview_a11y_updates(
        ctx: &egui::Context,
        pending: &mut HashMap<WebViewId, accesskit::TreeUpdate>,
    ) {
        if pending.is_empty() {
            return;
        }

        for (webview_id, tree_update) in pending.drain() {
            let anchor_id = Self::webview_accessibility_anchor_id(webview_id);
            let label = Self::webview_accessibility_label(webview_id, &tree_update);
            ctx.accesskit_node_builder(anchor_id, |builder| {
                builder.set_role(egui::accesskit::Role::Document);
                builder.set_label(label);
            });

            if tree_update.nodes.is_empty() {
                warn!(
                    "WebView accessibility injection used degraded synthesized document node for {:?}: incoming tree update had no nodes",
                    webview_id
                );
            }
        }
    }
}


#[cfg(test)]
#[path = "gui_tests.rs"]
mod gui_tests;


#[cfg(test)]
fn graph_intents_from_semantic_events(events: Vec<GraphSemanticEvent>) -> Vec<GraphIntent> {
    semantic_event_pipeline::graph_intents_from_semantic_events(events)
}

#[cfg(test)]
fn graph_intents_and_responsive_from_events(
    events: Vec<GraphSemanticEvent>,
) -> (Vec<GraphIntent>, Vec<WebViewId>, HashSet<WebViewId>) {
    semantic_event_pipeline::graph_intents_and_responsive_from_events(events)
}

fn refresh_graph_search_matches(
    graph_app: &GraphBrowserApp,
    query: &str,
    matches: &mut Vec<NodeKey>,
    active_index: &mut Option<usize>,
) {
    if query.trim().is_empty() {
        matches.clear();
        *active_index = None;
        return;
    }

    *matches = fuzzy_match_node_keys(&graph_app.workspace.graph, query);
    if matches.is_empty() {
        *active_index = None;
    } else if active_index.is_none_or(|idx| idx >= matches.len()) {
        *active_index = Some(0);
    }
}

fn step_graph_search_active_match(
    matches: &[NodeKey],
    active_index: &mut Option<usize>,
    step: isize,
) {
    if matches.is_empty() {
        *active_index = None;
        return;
    }

    let current = active_index.unwrap_or(0) as isize;
    let len = matches.len() as isize;
    let next = (current + step).rem_euclid(len) as usize;
    *active_index = Some(next);
}

fn active_graph_search_match(matches: &[NodeKey], active_index: Option<usize>) -> Option<NodeKey> {
    let idx = active_index?;
    matches.get(idx).copied()
}

#[cfg(test)]
mod accessibility_bridge_tests {
    use std::collections::HashMap;

    use super::Gui;
    use accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate};
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn webview_a11y_anchor_id_is_stable_per_webview() {
        let id = test_webview_id();
        let a = Gui::webview_accessibility_anchor_id(id);
        let b = Gui::webview_accessibility_anchor_id(id);
        assert_eq!(a, b);
    }

    #[test]
    fn webview_accessibility_label_prefers_focused_node_label() {
        let webview_id = test_webview_id();
        let mut focused = Node::new(Role::Document);
        focused.set_label("Focused title".to_string());
        let mut other = Node::new(Role::Paragraph);
        other.set_label("Other title".to_string());

        let update = TreeUpdate {
            nodes: vec![(NodeId(1), other), (NodeId(2), focused)],
            tree: Some(Tree::new(NodeId(1))),
            tree_id: TreeId::ROOT,
            focus: NodeId(2),
        };

        let label = Gui::webview_accessibility_label(webview_id, &update);
        assert!(label.contains("Focused title"));
    }

    #[test]
    fn webview_accessibility_label_falls_back_when_no_labels_exist() {
        let webview_id = test_webview_id();
        let update = TreeUpdate {
            nodes: vec![(NodeId(5), Node::new(Role::Document))],
            tree: Some(Tree::new(NodeId(5))),
            tree_id: TreeId::ROOT,
            focus: NodeId(5),
        };

        let label = Gui::webview_accessibility_label(webview_id, &update);
        assert!(label.contains("Embedded web content"));
        assert!(label.contains("1 accessibility node update"));
    }

    #[test]
    fn inject_webview_a11y_updates_drains_pending_map() {
        let webview_id = test_webview_id();
        let mut update_node = Node::new(Role::Document);
        update_node.set_label("Injected title".to_string());
        let update = TreeUpdate {
            nodes: vec![(NodeId(9), update_node)],
            tree: Some(Tree::new(NodeId(9))),
            tree_id: TreeId::ROOT,
            focus: NodeId(9),
        };

        let mut pending = HashMap::from([(webview_id, update)]);
        let ctx = egui::Context::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            Gui::inject_webview_a11y_updates(ctx, &mut pending);
        });

        assert!(
            pending.is_empty(),
            "bridge injection should consume pending webview accessibility updates"
        );
    }
}

#[cfg(all(test, feature = "diagnostics"))]
mod tool_pane_routing_tests {
    use super::Gui;
    use crate::shell::desktop::workbench::pane_model::ToolPaneState;
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use egui_tiles::{Tile, Tiles, Tree};

    fn diagnostics_active(tree: &Tree<TileKind>) -> bool {
        tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
            )
        })
    }

    #[test]
    fn diagnostics_shortcut_focuses_existing_diagnostics_tool_pane() {
        let mut tiles = Tiles::default();
        let settings_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
        let diagnostics_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
        let tabs_root = tiles.insert_tab_tile(vec![settings_id, diagnostics_id]);
        let mut tree = Tree::new("tool_tabs", tabs_root, tiles);

        let _ = tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Settings)))
        });
        assert!(!diagnostics_active(&tree));

        Gui::open_or_focus_diagnostics_tool_pane(&mut tree);
        assert!(diagnostics_active(&tree));
    }

    #[test]
    fn diagnostics_shortcut_inserts_diagnostics_tool_pane_when_missing() {
        let mut tiles = Tiles::default();
        let settings_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
        let mut tree = Tree::new("tool_tabs", settings_id, tiles);

        Gui::open_or_focus_diagnostics_tool_pane(&mut tree);

        let diagnostics_count = tree
            .tiles
            .iter()
            .filter(|(_, tile)| {
                matches!(
                    tile,
                    Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics))
                )
            })
            .count();
        assert_eq!(diagnostics_count, 1);
        assert!(diagnostics_active(&tree));
    }

    #[test]
    fn multiple_tool_panes_coexist_with_expected_titles() {
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
        let mut tree = Tree::new("tool_tabs", root, tiles);

        Gui::open_or_focus_tool_pane(&mut tree, ToolPaneState::HistoryManager);
        Gui::open_or_focus_tool_pane(&mut tree, ToolPaneState::AccessibilityInspector);
        Gui::open_or_focus_tool_pane(&mut tree, ToolPaneState::Settings);

        let mut tool_titles: Vec<&'static str> = tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Tool(tool)) => Some(tool.title()),
                _ => None,
            })
            .collect();
        tool_titles.sort_unstable();

        assert_eq!(
            tool_titles,
            vec!["Accessibility", "Diagnostics", "History", "Settings"]
        );
    }

    #[test]
    fn diagnostics_shortcut_focuses_diagnostics_not_other_tool_pane() {
        let mut tiles = Tiles::default();
        let history_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::HistoryManager));
        let diagnostics_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
        let tabs_root = tiles.insert_tab_tile(vec![history_id, diagnostics_id]);
        let mut tree = Tree::new("tool_tabs", tabs_root, tiles);

        let _ = tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::HistoryManager)))
        });

        Gui::open_or_focus_diagnostics_tool_pane(&mut tree);

        assert!(tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
            )
        }));
        assert!(!tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::Tool(ToolPaneState::HistoryManager)))
            )
        }));
    }
}

#[cfg(test)]
mod graph_split_intent_tests {
    use super::Gui;
    use crate::app::{GraphIntent, GraphViewId};
    use crate::shell::desktop::workbench::pane_model::{PaneId, SplitDirection};
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use egui_tiles::{Tile, Tiles, Tree};

    fn active_graph_count(tree: &Tree<TileKind>) -> usize {
        tree.active_tiles()
            .into_iter()
            .filter(|tile_id| matches!(tree.tiles.get(*tile_id), Some(Tile::Pane(TileKind::Graph(_)))))
            .count()
    }

    #[test]
    fn split_pane_intent_creates_new_graph_view_pane() {
        let initial_view = GraphViewId::new();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(initial_view));
        let mut tree = Tree::new("graphshell_tiles", root, tiles);

        let mut intents = vec![GraphIntent::SplitPane {
            source_pane: PaneId::new(),
            direction: SplitDirection::Horizontal,
        }];

        Gui::handle_tool_pane_intents(&mut tree, &mut intents);

        assert!(intents.is_empty(), "split intent should be consumed by workbench authority");

        let graph_views: Vec<GraphViewId> = tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Graph(view_id)) => Some(*view_id),
                _ => None,
            })
            .collect();

        assert_eq!(graph_views.len(), 2, "split should produce a second graph pane");
        assert!(graph_views.contains(&initial_view));
        assert!(graph_views.iter().any(|view_id| *view_id != initial_view));
        assert!(active_graph_count(&tree) >= 1, "a graph pane should remain active");
    }
}

#[cfg(test)]
fn graph_intent_for_thumbnail_result(
    graph_app: &GraphBrowserApp,
    result: &ThumbnailCaptureResult,
) -> Option<GraphIntent> {
    thumbnail_pipeline::graph_intent_for_thumbnail_result(graph_app, result)
}
