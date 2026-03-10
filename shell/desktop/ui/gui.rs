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
use super::gui_state::{GuiRuntimeState, ToolbarDraft, ToolbarState};
use super::persistence_ops;
#[cfg(test)]
use super::thumbnail_pipeline;
use crate::app::{
    BrowserCommand, BrowserCommandTarget, GraphBrowserApp, GraphIntent, GraphViewId,
    ToastAnchorPreference,
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
    CHANNEL_UX_NAVIGATION_TRANSITION, RegistryRuntime, phase3_subscribe_signal,
    phase3_unsubscribe_signal,
};
use crate::shell::desktop::runtime::registries::signal_routing::{
    LifecycleSignal, ObserverId, SignalKind, SignalTopic,
};
use crate::shell::desktop::ui::thumbnail_pipeline::{
    RendererFaviconTextureCache, ThumbnailCaptureResult,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::workbench::tile_compositor;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode};
use crate::util::CoordBridge;

#[path = "gui/accessibility.rs"]
mod accessibility;
#[path = "gui/accesskit_events.rs"]
mod accesskit_events;
#[path = "gui/accesskit_input.rs"]
mod accesskit_input;
#[path = "gui/focus_state.rs"]
mod focus_state;
#[path = "gui/gui_update_coordinator.rs"]
mod gui_update_coordinator;
#[path = "gui/hit_testing.rs"]
mod hit_testing;
#[path = "gui/input_routing.rs"]
mod input_routing;
#[cfg(test)]
#[path = "gui/intent_translation.rs"]
mod intent_translation;
#[path = "gui/interaction_queries.rs"]
mod interaction_queries;
#[path = "gui/paint_pass.rs"]
mod paint_pass;
#[path = "gui/pane_queries.rs"]
mod pane_queries;
#[path = "gui/startup.rs"]
mod startup;
#[path = "gui/toolbar_status_sync.rs"]
mod toolbar_status_sync;
#[path = "gui/tree_bootstrap.rs"]
mod tree_bootstrap;
#[path = "gui/update_frame_phases.rs"]
mod update_frame_phases;
#[path = "gui/window_input.rs"]
mod window_input;

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

    /// Renderer-local favicon texture cache keyed by ephemeral WebViewId.
    ///
    /// Durable favicon ownership remains on the node model; this cache only keeps
    /// uploaded egui textures alive across draw calls until node-keyed consumers refresh.
    renderer_favicon_textures: RendererFaviconTextureCache,

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

    /// Pending lifecycle notifications for semantic-index-driven lens refresh.
    semantic_index_signal_rx: Receiver<usize>,
    semantic_index_signal_observer: ObserverId,

    /// Tokio runtime for async background workers
    tokio_runtime: tokio::runtime::Runtime,

    /// Async worker supervision and intent queue
    control_panel: ControlPanel,
}

impl Drop for Gui {
    fn drop(&mut self) {
        let _ = phase3_unsubscribe_signal(
            SignalTopic::Lifecycle,
            self.semantic_index_signal_observer,
        );
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

        let (mut graph_app, tiles_tree, initial_search_filter_mode) =
            startup::initialize_startup_graph_and_tiles(
                graph_data_dir,
                &initial_url,
                graph_snapshot_interval_secs,
            );
        let (thumbnail_capture_tx, thumbnail_capture_rx) = channel();

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
        let registry_runtime = RegistryRuntime::new_with_mods();
        let (semantic_index_signal_tx, semantic_index_signal_rx) = channel();
        let semantic_index_signal_observer =
            phase3_subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if let SignalKind::Lifecycle(LifecycleSignal::SemanticIndexUpdated {
                    indexed_nodes,
                }) = &signal.kind
                {
                    let _ = semantic_index_signal_tx.send(*indexed_nodes);
                }
                Ok(())
            });

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
            renderer_favicon_textures: Default::default(),
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
                active_toolbar_pane: None,
                toolbar_drafts: HashMap::new(),
                command_palette_toggle_requested: false,
                deferred_open_child_webviews: Vec::new(),
            },
            #[cfg(feature = "diagnostics")]
            diagnostics_state: diagnostics::DiagnosticsState::new(),
            registry_runtime,
            semantic_index_signal_rx,
            semantic_index_signal_observer,
            tokio_runtime,
            control_panel,
        }
    }

    #[cfg(test)]
    fn parse_data_dir_input(raw: &str) -> Option<PathBuf> {
        persistence_ops::parse_data_dir_input(raw)
    }

    pub(crate) fn is_graph_view(&self) -> bool {
        !pane_queries::tree_has_active_node_pane(&self.tiles_tree)
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
        window_input::on_window_event(self, winit_window, event)
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

    fn apply_pending_semantic_index_updates(&mut self) {
        let mut saw_update = false;
        while self.semantic_index_signal_rx.try_recv().is_ok() {
            saw_update = true;
        }
        if saw_update {
            self.graph_app.refresh_registry_backed_view_lenses();
        }
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

    fn persist_active_toolbar_draft(&mut self) {
        let Some(active_pane) = self.runtime_state.active_toolbar_pane else {
            return;
        };
        self.runtime_state.toolbar_drafts.insert(
            active_pane,
            ToolbarDraft::from_toolbar_state(&self.toolbar_state),
        );
    }

    fn sync_active_toolbar_draft(&mut self, window: &EmbedderWindow) {
        let next_active_pane = window.focused_pane();
        if self.runtime_state.active_toolbar_pane == next_active_pane {
            return;
        }

        self.persist_active_toolbar_draft();
        self.runtime_state.active_toolbar_pane = next_active_pane;

        let Some(active_pane) = next_active_pane else {
            return;
        };

        let draft = self
            .runtime_state
            .toolbar_drafts
            .entry(active_pane)
            .or_insert_with(|| ToolbarDraft::from_toolbar_state(&self.toolbar_state))
            .clone();
        draft.apply_to_toolbar_state(&mut self.toolbar_state);
    }

    pub(crate) fn request_browser_command(
        &mut self,
        target: BrowserCommandTarget,
        command: BrowserCommand,
    ) {
        self.graph_app.request_browser_command(target, command);
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
        self.apply_pending_semantic_index_updates();

        let GuiUpdateInput {
            state,
            window,
            headed_window,
        } = input;
        self.sync_active_toolbar_draft(window);
        // Note: We need Rc<RunningAppState> for runtime viewer creation, but this method
        // is called from trait methods that only provide &RunningAppState.
        // The caller should have Rc available at the call site.
        self.rendering_context
            .make_current()
            .expect("Could not make RenderingContext current");
        tree_bootstrap::ensure_tiles_tree_root(&mut self.tiles_tree);
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
            renderer_favicon_textures,
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
            active_toolbar_pane: _,
            toolbar_drafts: _,
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
                favicon_textures: renderer_favicon_textures,
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

        self.persist_active_toolbar_draft();

        GuiUpdateOutput
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
        self.sync_active_toolbar_draft(window);
        // Note: We must use the "bitwise OR" (|) operator here instead of "logical OR" (||)
        //       because logical OR would short-circuit if any of the functions return true.
        //       We want to ensure that all functions are called. The "bitwise OR" operator
        //       does not short-circuit.
        let changed = input_routing::collect_webview_update_flags(self, window);
        self.persist_active_toolbar_draft();
        changed
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
) -> (Vec<GraphIntent>, HashSet<WebViewId>) {
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
