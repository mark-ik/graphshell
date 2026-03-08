/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use arboard::Clipboard;
use egui_tiles::{Tile, TileId, Tiles, Tree};
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

use super::graph_search_flow;
use super::gui_frame;
use super::gui_orchestration;
use super::gui_state::{GuiRuntimeState, ToolbarState};
use super::persistence_ops;
#[cfg(test)]
use super::thumbnail_pipeline;
use crate::app::{
    GraphBrowserApp, GraphIntent, GraphViewId, SearchDisplayMode, ToastAnchorPreference,
};
use crate::graph::NodeKey;
use crate::shell::desktop::host::event_loop::AppEvent;
use crate::shell::desktop::host::headed_window;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
#[cfg(test)]
use crate::shell::desktop::host::window::GraphSemanticEvent;
#[cfg(test)]
use crate::shell::desktop::lifecycle::semantic_event_pipeline;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::render_backend::{
    UiRenderBackendContract, UiRenderBackendHandle, create_ui_render_backend,
};
use crate::shell::desktop::runtime::control_panel::ControlPanel;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_NAVIGATION_TRANSITION, RegistryRuntime, knowledge,
};
use crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailCaptureResult;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::workbench::tile_compositor;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode};
use crate::util::CoordBridge;

#[path = "gui/gui_update_coordinator.rs"]
mod gui_update_coordinator;
#[path = "gui/accessibility.rs"]
mod accessibility;
#[path = "gui/update_frame_phases.rs"]
mod update_frame_phases;
#[path = "gui/startup.rs"]
mod startup;
#[path = "gui/focus_state.rs"]
mod focus_state;
#[path = "gui/toolbar_status_sync.rs"]
mod toolbar_status_sync;
#[path = "gui/hit_testing.rs"]
mod hit_testing;
#[path = "gui/accesskit_input.rs"]
mod accesskit_input;
#[path = "gui/pane_queries.rs"]
mod pane_queries;
#[path = "gui/input_routing.rs"]
mod input_routing;
#[path = "gui/accesskit_events.rs"]
mod accesskit_events;
#[path = "gui/paint_pass.rs"]
mod paint_pass;
#[path = "gui/interaction_queries.rs"]
mod interaction_queries;
#[cfg(test)]
#[path = "gui/intent_translation.rs"]
mod intent_translation;

use accessibility::WebViewA11yGraftPlan;
use update_frame_phases::ExecuteUpdateFrameArgs;

#[cfg(test)]
use update_frame_phases::UpdateFrameStage;

pub(crate) struct GuiUpdateInput<'a> {
    pub(crate) state: &'a RunningAppState,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) headed_window: &'a headed_window::HeadedWindow,
}

pub(crate) struct GuiUpdateOutput;

/// The user interface of a headed Graphshell runtime. Currently this is implemented via
/// egui.
pub struct Gui {
    rendering_context: Rc<OffscreenRenderingContext>,
    window_rendering_context: Rc<WindowRenderingContext>,
    context: UiRenderBackendHandle,
    /// Live workbench layout authority.
    ///
    /// This `egui_tiles::Tree<TileKind>` is the canonical runtime pane tree.
    /// `pane_model` defines the payload/schema carried by this tree, but does
    /// not own a separate competing retained layout tree.
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

    /// Per-node offscreen rendering contexts for composited node-viewer tiles.
    tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>>,

    /// Per-node favicon textures for egui_tiles tab rendering.
    tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)>,

    /// Sender for asynchronous runtime viewer thumbnail capture results.
    thumbnail_capture_tx: Sender<ThumbnailCaptureResult>,

    /// Receiver for asynchronous runtime viewer thumbnail capture results.
    thumbnail_capture_rx: Receiver<ThumbnailCaptureResult>,

    /// Runtime viewers with an in-flight thumbnail request.
    thumbnail_capture_in_flight: HashSet<WebViewId>,

    /// Runtime backpressure state for tile-driven viewer creation retries.
    webview_creation_backpressure: HashMap<NodeKey, WebviewCreationBackpressureState>,

    /// Pending accessibility tree updates received from runtime viewers that have
    /// not yet been injected into egui's accessibility tree. Keyed by WebViewId
    /// so that a newer update from the same runtime viewer supersedes the previous one.
    pending_webview_a11y_updates: HashMap<WebViewId, accesskit::TreeUpdate>,

    /// Cached reference to RunningAppState for runtime viewer creation.
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
        self.context.destroy_surface();
    }
}

fn apply_node_focus_state(runtime_state: &mut GuiRuntimeState, node_key: Option<NodeKey>) {
    focus_state::apply_node_focus_state(runtime_state, node_key)
}

fn apply_graph_surface_focus_state(
    runtime_state: &mut GuiRuntimeState,
    graph_app: &mut GraphBrowserApp,
    active_graph_view: Option<GraphViewId>,
) {
    focus_state::apply_graph_surface_focus_state(runtime_state, graph_app, active_graph_view)
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
        let mut context = create_ui_render_backend(event_loop, rendering_context.glow_gl_api());

        context.init_surface_accesskit(event_loop, winit_window, event_loop_proxy);
        winit_window.set_visible(true);

        context.egui_context_mut().options_mut(|options| {
            // Disable the builtin egui handlers for the Ctrl+Plus, Ctrl+Minus and Ctrl+0
            // shortcuts as they don't work well with Graphshell's `device-pixel-ratio` CLI argument.
            options.zoom_with_keyboard = false;

            // On platforms where winit fails to obtain a system theme, fall back to a light theme
            // since it is the more common default.
            options.fallback_theme = egui::Theme::Light;
        });

        let initial_data_dir = graph_data_dir
            .unwrap_or_else(crate::services::persistence::GraphStore::default_data_dir);
        let mut graph_app = GraphBrowserApp::new_from_dir(initial_data_dir.clone());
        if let Some(snapshot_secs) = graph_snapshot_interval_secs
            && let Err(e) = graph_app.set_snapshot_interval_secs(snapshot_secs)
        {
            warn!("Failed to apply snapshot interval from startup preferences: {e}");
        }
        let mut tiles = Tiles::default();
        let graph_tile_id = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let mut tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);

        let _ = startup::restore_startup_session_frame_if_available(&mut graph_app, &mut tiles_tree);

        // Only create initial node if graph wasn't recovered from persistence
        if !graph_app.has_recovered_graph() {
            use euclid::default::Point2D;
            graph_app.apply_reducer_intents([GraphIntent::CreateNodeAtUrl {
                url: initial_url.to_string(),
                position: Point2D::new(400.0, 300.0),
            }]);
        }
        let membership_index =
            persistence_ops::build_membership_index_from_frame_manifests(&graph_app);
        graph_app.init_membership_index(membership_index);
        let (workspace_recency, workspace_activation_seq) =
            persistence_ops::build_frame_activation_recency_from_frame_manifests(&graph_app);
        graph_app.init_frame_activation_recency(workspace_recency, workspace_activation_seq);
        let (thumbnail_capture_tx, thumbnail_capture_rx) = channel();
        let initial_search_filter_mode = matches!(
            graph_app.workspace.search_display_mode,
            SearchDisplayMode::Filter
        );

        // Create tokio runtime for background workers
        let tokio_runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime for async workers");

        // Initialize ControlPanel and spawn workers inside runtime context
        let control_panel = {
            let _guard = tokio_runtime.enter();
            let mut panel = ControlPanel::new();
            panel.spawn_memory_monitor();
            panel.spawn_mod_loader();
            panel.spawn_prefetch_scheduler();
            // Spawn sync worker if Verse mod is available.
            panel.spawn_p2p_sync_worker();
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
                .with_anchor(Self::toast_anchor(
                    graph_app.workspace.toast_anchor_preference,
                ))
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
                focused_node_hint: None,
                graph_surface_focused: false,
                focus_ring_node_key: None,
                focus_ring_started_at: None,
                focus_ring_duration: Duration::from_millis(500),
                omnibar_search_session: None,
                command_palette_toggle_requested: false,
                deferred_open_child_webviews: Vec::new(),
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

    pub(crate) fn is_graph_view(&self) -> bool {
        !self.has_active_node_pane()
    }

    /// Set the RunningAppState reference for runtime viewer creation.
    pub(crate) fn set_state(&mut self, state: Rc<RunningAppState>) {
        self.state = Some(state);
    }

    pub(crate) fn surrender_focus(&self) {
        self.context.egui_context().memory_mut(|memory| {
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
        let mut response = self.context.handle_window_event(winit_window, event);

        // When no node-viewer tile is active, consume user input events so they
        // never reach an inactive/hidden runtime viewer.
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
                }
                _ => {}
            }
        }

        response
    }

    /// The height of the top toolbar, i.e. distance from the top of the window
    /// to the runtime viewer region.
    pub(crate) fn toolbar_height(&self) -> Length<f32, DeviceIndependentPixel> {
        self.toolbar_height
    }

    pub(crate) fn webview_at_point(
        &self,
        point: Point2D<f32, DeviceIndependentPixel>,
    ) -> Option<(WebViewId, Point2D<f32, DeviceIndependentPixel>)> {
        hit_testing::webview_at_point(&self.tiles_tree, &self.graph_app, point)
    }

    pub(crate) fn graph_at_point(&self, point: Point2D<f32, DeviceIndependentPixel>) -> bool {
        hit_testing::graph_at_point(&self.tiles_tree, point)
    }

    pub(crate) fn focused_node_key(&self) -> Option<NodeKey> {
        interaction_queries::focused_node_key(self)
    }

    pub(crate) fn has_focused_node(&self) -> bool {
        interaction_queries::has_focused_node(self)
    }

    pub(crate) fn webview_id_for_node_key(&self, node_key: NodeKey) -> Option<WebViewId> {
        interaction_queries::webview_id_for_node_key(self, node_key)
    }

    #[allow(dead_code)]
    pub(crate) fn active_tile_webview_id(&self) -> Option<WebViewId> {
        interaction_queries::active_tile_webview_id(self)
    }

    pub(crate) fn set_focused_node_key(&mut self, node_key: Option<NodeKey>) {
        apply_node_focus_state(&mut self.runtime_state, node_key);
    }

    pub(crate) fn node_key_for_webview_id(&self, webview_id: WebViewId) -> Option<NodeKey> {
        interaction_queries::node_key_for_webview_id(self, webview_id)
    }

    pub(crate) fn focus_graph_surface(&mut self) {
        apply_graph_surface_focus_state(
            &mut self.runtime_state,
            &mut self.graph_app,
            tile_view_ops::active_graph_view_id(&self.tiles_tree),
        );
    }

    pub(crate) fn location_has_focus(&self) -> bool {
        interaction_queries::location_has_focus(self)
    }

    pub(crate) fn request_location_submit(&mut self) {
        interaction_queries::request_location_submit(self)
    }

    pub(crate) fn request_command_palette_toggle(&mut self) {
        interaction_queries::request_command_palette_toggle(self)
    }

    pub(crate) fn egui_wants_keyboard_input(&self) -> bool {
        interaction_queries::egui_wants_keyboard_input(self)
    }

    pub(crate) fn egui_wants_pointer_input(&self) -> bool {
        interaction_queries::egui_wants_pointer_input(self)
    }

    pub(crate) fn pointer_hover_position(&self) -> Option<Point2D<f32, DeviceIndependentPixel>> {
        interaction_queries::pointer_hover_position(self)
    }

    pub(crate) fn ui_overlay_active(&self) -> bool {
        interaction_queries::ui_overlay_active(self)
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
        // Note: We need Rc<RunningAppState> for runtime viewer creation, but this method
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
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            omnibar_search_session,
            command_palette_toggle_requested,
            deferred_open_child_webviews,
        } = runtime_state;

        let winit_window = headed_window.winit_window();
        Self::configure_frame_toasts(toasts, graph_app.workspace.toast_anchor_preference);
        context.run_ui_frame(winit_window, |ctx| {
            Self::execute_update_frame(ExecuteUpdateFrameArgs {
                ctx,
                winit_window,
                state,
                window,
                headed_window,
                graph_app,
                pending_webview_a11y_updates,
                tiles_tree,
                toolbar_height,
                toolbar_state,
                toasts,
                clipboard,
                favicon_textures,
                tile_rendering_contexts,
                tile_favicon_textures,
                thumbnail_capture_tx,
                thumbnail_capture_rx,
                thumbnail_capture_in_flight,
                webview_creation_backpressure,
                app_state,
                graph_search_open,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
                focused_node_hint,
                graph_surface_focused,
                focus_ring_node_key,
                focus_ring_started_at,
                focus_ring_duration,
                omnibar_search_session,
                command_palette_toggle_requested,
                deferred_open_child_webviews,
                rendering_context,
                window_rendering_context,
                registry_runtime,
                control_panel,
                #[cfg(feature = "diagnostics")]
                diagnostics_state,
            });
        });

        GuiUpdateOutput
    }

    fn ensure_tiles_tree_root(&mut self) {
        if self.tiles_tree.root().is_none() {
            let graph_tile_id = Self::insert_default_graph_tile(&mut self.tiles_tree);
            Self::set_tiles_tree_root(&mut self.tiles_tree, graph_tile_id);
        }
    }

    fn insert_default_graph_tile(tiles_tree: &mut Tree<TileKind>) -> TileId {
        tiles_tree
            .tiles
            .insert_pane(TileKind::Graph(GraphViewId::default()))
    }

    fn set_tiles_tree_root(tiles_tree: &mut Tree<TileKind>, root_tile_id: TileId) {
        tiles_tree.root = Some(root_tile_id);
    }

    #[cfg(feature = "diagnostics")]
    fn open_or_focus_tool_pane(
        tiles_tree: &mut Tree<TileKind>,
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    ) {
        tile_view_ops::open_or_focus_tool_pane(tiles_tree, kind);
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

    fn has_active_node_pane(&self) -> bool {
        pane_queries::tree_has_active_node_pane(&self.tiles_tree)
    }

    fn tree_has_active_node_pane(tiles_tree: &Tree<TileKind>) -> bool {
        pane_queries::tree_has_active_node_pane(tiles_tree)
    }

    fn reconcile_workspace_graph_views_from_tiles(
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &Tree<TileKind>,
    ) {
        pane_queries::reconcile_workspace_graph_views_from_tiles(graph_app, tiles_tree);
    }

    /// Paint the GUI, as of the last update.
    pub(crate) fn paint(&mut self, window: &Window) {
        paint_pass::paint(self, window);
    }

    /// Updates the location field from the given [`RunningAppState`], unless the user has started
    /// editing it without clicking Go, returning true iff it has changed (needing an egui update).
    fn update_location_in_toolbar(
        &mut self,
        window: &EmbedderWindow,
        focused_node_key: Option<NodeKey>,
    ) -> bool {
        if input_routing::should_skip_toolbar_location_sync(&self.toolbar_state) {
            // Preserve active omnibar node-search query text while cycling matches.
            return false;
        }

        let has_node_panes = input_routing::has_any_node_panes(&self.tiles_tree);
        toolbar_status_sync::update_location_in_toolbar(
            &self.graph_app,
            &mut self.toolbar_state,
            has_node_panes,
            focused_node_key,
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
        input_routing::collect_webview_update_flags(self, window)
    }

    /// Returns true if a redraw is required after handling the provided event.
    pub(crate) fn handle_accesskit_event(
        &mut self,
        event: &egui_winit::accesskit_winit::WindowEvent,
    ) -> bool {
        accesskit_events::handle_accesskit_event(self, event)
    }

    pub(crate) fn set_zoom_factor(&self, factor: f32) {
        let clamped = accesskit_input::clamp_zoom_factor(factor);
        self.context.egui_context().set_zoom_factor(clamped);
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
        // Store the most recent update per runtime viewer; it will be injected into
        // egui's accessibility tree at the start of the next frame inside
        // the context.run() callback.
        self.pending_webview_a11y_updates
            .insert(webview_id, tree_update);
    }

    fn webview_accessibility_anchor_id(webview_id: WebViewId) -> egui::Id {
        accessibility::webview_accessibility_anchor_id(webview_id)
    }

    fn webview_accessibility_label(
        webview_id: WebViewId,
        tree_update: &accesskit::TreeUpdate,
    ) -> String {
        accessibility::webview_accessibility_label(webview_id, tree_update)
    }

    fn build_webview_a11y_graft_plan(
        webview_id: WebViewId,
        tree_update: &accesskit::TreeUpdate,
    ) -> WebViewA11yGraftPlan {
        accessibility::build_webview_a11y_graft_plan(webview_id, tree_update)
    }

    /// Inject pending runtime viewer accessibility tree updates into egui's
    /// accessibility tree.
    ///
    /// For each node in a Servo-provided `accesskit::TreeUpdate`, this bridge
    /// synthesizes a deterministic egui `Id` and applies a compatibility
    /// conversion for role/label fields between AccessKit versions.
    ///
    /// Nodes whose `NodeId` is zero or `u64::MAX` (egui's root sentinel) are
    /// skipped to avoid collisions with egui's own accessibility tree.
    fn inject_webview_a11y_updates(
        ctx: &egui::Context,
        pending: &mut HashMap<WebViewId, accesskit::TreeUpdate>,
    ) {
        accessibility::inject_webview_a11y_updates(ctx, pending);
    }
}

fn ui_overlay_active_from_flags(
    show_command_palette: bool,
    show_help_panel: bool,
    show_radial_menu: bool,
    show_clear_data_confirm: bool,
) -> bool {
    focus_state::ui_overlay_active_from_flags(
        show_command_palette,
        show_help_panel,
        show_radial_menu,
        show_clear_data_confirm,
    )
}

#[cfg(test)]
#[path = "gui_tests.rs"]
mod gui_tests;

#[cfg(test)]
fn graph_intents_from_semantic_events(events: Vec<GraphSemanticEvent>) -> Vec<GraphIntent> {
    intent_translation::graph_intents_from_semantic_events(events)
}

#[cfg(test)]
fn graph_intents_and_responsive_from_events(
    events: Vec<GraphSemanticEvent>,
) -> (Vec<GraphIntent>, Vec<WebViewId>, HashSet<WebViewId>) {
    intent_translation::graph_intents_and_responsive_from_events(events)
}

#[cfg(test)]
#[path = "gui/accessibility_bridge_tests.rs"]
mod accessibility_bridge_tests;

#[cfg(all(test, feature = "diagnostics"))]
#[path = "gui/tool_pane_routing_tests.rs"]
mod tool_pane_routing_tests;

#[cfg(test)]
#[path = "gui/graph_split_intent_tests.rs"]
mod graph_split_intent_tests;

#[cfg(test)]
fn graph_intent_for_thumbnail_result(
    graph_app: &GraphBrowserApp,
    result: &ThumbnailCaptureResult,
) -> Option<GraphIntent> {
    thumbnail_pipeline::graph_intent_for_thumbnail_result(graph_app, result)
}

#[cfg(all(test, feature = "diagnostics"))]
#[path = "gui/diagnostics_tests.rs"]
mod diagnostics_tests;
