/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use arboard::Clipboard;
use egui_tiles::{Tile, TileId, Tiles, Tree};
use egui_winit::EventResponse;
use euclid::{Length, Point2D};
use log::warn;
use servo::{
    DeviceIndependentPixel, LoadStatus, OffscreenRenderingContext, WebViewId,
    WindowRenderingContext,
};
use url::Url;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

use super::graph_search_flow;
use super::gui_frame;
use super::gui_orchestration;
use super::gui_state::{
    GuiRuntimeState, LocalFocusTarget, PaneRegionHint, RuntimeFocusAuthorityState,
    RuntimeFocusInputs, RuntimeFocusInspector, RuntimeFocusState, ToolbarDraft, ToolbarState,
};
use super::toolbar_routing::{self, ToolbarNavAction};
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
    UiHostRenderBootstrap, UiRenderBackendContract, UiRenderBackendHandle, UiRenderBackendInit,
    activate_ui_render_backend, create_ui_render_backend,
};
use crate::shell::desktop::runtime::control_panel::ControlPanel;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::nip07_bridge;
use crate::shell::desktop::runtime::registries::workbench_surface;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM, CHANNEL_UX_NAVIGATION_TRANSITION, RegistryRuntime,
    phase3_resolve_active_theme, phase3_shared_runtime,
};
use crate::shell::desktop::ui::thumbnail_pipeline::{
    RendererFaviconTextureCache, ThumbnailCaptureResult,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::workbench::tile_compositor;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode};
use crate::util::CoordBridge;

#[path = "gui/accessibility.rs"]
mod accessibility;
#[cfg(test)]
pub(crate) use accessibility::selected_node_affordance_projection_from_annotations;
pub(crate) use accessibility::{
    TileAffordanceAccessibilityProjection, WebViewAccessibilityBridgeHealthSnapshot,
};
#[path = "gui/accesskit_events.rs"]
mod accesskit_events;
#[path = "gui/accesskit_input.rs"]
mod accesskit_input;
#[path = "gui/focus_state.rs"]
mod focus_state;
#[path = "gui/frame_inbox.rs"]
mod frame_inbox;
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
use frame_inbox::GuiFrameInbox;

#[allow(unused_imports)]
pub(crate) use focus_state::{
    apply_focus_command, apply_graph_search_local_focus_state,
    capture_command_surface_return_target_in_authority,
    capture_tool_surface_return_target_in_authority, desired_runtime_focus_state,
    realize_embedded_content_focus_from_authority, refresh_realized_runtime_focus_state,
    runtime_focus_inspector, seed_command_surface_return_target_from_authority,
    seed_tool_surface_return_target_from_authority,
    seed_transient_surface_return_target_from_authority, semantic_region_for_tool_surface_target,
};
pub(crate) use focus_state::{workbench_runtime_focus_state, workspace_runtime_focus_state};
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
    pending_webview_a11y_updates: HashMap<WebViewId, servo::accesskit::TreeUpdate>,

    /// Cached reference to RunningAppState for runtime viewer creation.
    state: Option<Rc<RunningAppState>>,

    /// Runtime UI state used by the frame coordinator and toolbar/search flows.
    runtime_state: GuiRuntimeState,

    #[cfg(feature = "diagnostics")]
    diagnostics_state: diagnostics::DiagnosticsState,

    /// Registry runtime for semantic services
    registry_runtime: Arc<RegistryRuntime>,

    /// Typed frame-bound relay set for Shell-facing async signal bridges.
    frame_inbox: GuiFrameInbox,

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

        activate_ui_render_backend(self.rendering_context.as_ref());
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

fn apply_pane_activation_focus_state(runtime_state: &mut GuiRuntimeState, pane_id: Option<PaneId>) {
    focus_state::apply_pane_activation_focus_state(runtime_state, pane_id)
}

fn clear_embedded_content_focus(
    runtime_state: &mut GuiRuntimeState,
    graph_app: &mut GraphBrowserApp,
) {
    if graph_app.embedded_content_focus_webview().is_some() {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM,
            latency_us: 0,
        });
    }
    focus_state::apply_focus_command(
        &mut runtime_state.focus_authority,
        crate::shell::desktop::ui::gui_state::FocusCommand::SetEmbeddedContentFocus {
            target: None,
        },
    );
    focus_state::realize_embedded_content_focus_from_authority(
        &runtime_state.focus_authority,
        graph_app,
    );
}

fn apply_requested_settings_route_update(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) {
    if let Some(intent) =
        workbench_surface::handle_requested_settings_route(graph_app, tiles_tree, url)
    {
        graph_app.enqueue_workbench_intent(intent);
    }
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
        render_host: UiHostRenderBootstrap,
        initial_url: Url,
        graph_data_dir: Option<PathBuf>,
        graph_snapshot_interval_secs: Option<u64>,
        worker_idle_threshold_secs: Option<u64>,
    ) -> Self {
        let mut context = create_ui_render_backend(
            event_loop,
            UiRenderBackendInit {
                window: winit_window,
                render_host: &render_host,
            },
        );
        let (rendering_context, window_rendering_context) = render_host.into_contexts();

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

        let registry_runtime = phase3_shared_runtime();

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

        // Initialize ControlPanel with an explicit runtime handle so later
        // Shell relay setup does not depend on a temporary enter() guard.
        let mut control_panel =
            ControlPanel::new_with_runtime(worker_idle_threshold_secs, tokio_runtime.handle().clone());
        control_panel.spawn_memory_monitor();
        control_panel.spawn_mod_loader();
        control_panel.spawn_prefetch_scheduler();
        // Spawn sync worker if Verse mod is available.
        control_panel.spawn_p2p_sync_worker();
        control_panel.spawn_nostr_relay_worker(Arc::clone(&registry_runtime));
        if let Err(error) =
            control_panel.spawn_registered_agent("agent:tag_suggester", Arc::clone(&registry_runtime))
        {
            warn!("Failed to spawn tag suggester agent: {error}");
        }
        graph_app.set_sync_command_tx(control_panel.sync_command_sender());
        let frame_inbox = GuiFrameInbox::spawn(&mut control_panel);

        let mut gui = Self {
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
                    graph_app.workspace.chrome_ui.toast_anchor_preference,
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
                focus_authority: RuntimeFocusAuthorityState::default(),
                toolbar_drafts: HashMap::new(),
                command_palette_toggle_requested: false,
                pending_webview_context_surface_requests: Vec::new(),
                deferred_open_child_webviews: Vec::new(),
            },
            #[cfg(feature = "diagnostics")]
            diagnostics_state: diagnostics::DiagnosticsState::new(),
            registry_runtime,
            frame_inbox,
            tokio_runtime,
            control_panel,
        };
        gui.apply_runtime_theme_visuals();
        gui
    }

    pub(crate) fn try_handle_nip07_prompt(
        &mut self,
        webview_id: WebViewId,
        message: &str,
    ) -> Option<String> {
        nip07_bridge::try_handle_prompt_message(message, |request| {
            self.handle_nip07_bridge_request(webview_id, request)
        })
    }

    fn handle_nip07_bridge_request(
        &mut self,
        webview_id: WebViewId,
        request: nip07_bridge::Nip07BridgeRequest,
    ) -> nip07_bridge::Nip07BridgeResponse {
        let resolved_url = self
            .graph_app
            .get_node_for_webview(webview_id)
            .and_then(|node_key| {
                self.graph_app
                    .workspace
                    .domain
                    .graph
                    .get_node(node_key)
                    .map(|node| node.url().to_string())
            })
            .or(request.href);

        let context = resolved_url.unwrap_or_else(|| "<unknown>".to_string());
        let grants_before =
            crate::shell::desktop::runtime::registries::phase3_nostr_nip07_permission_grants();
        let result = crate::shell::desktop::runtime::registries::phase3_nostr_nip07_request(
            &context,
            &request.method,
            &request.params,
        );
        let grants_after =
            crate::shell::desktop::runtime::registries::phase3_nostr_nip07_permission_grants();
        if grants_before != grants_after {
            self.graph_app.save_persisted_nostr_nip07_permissions();
        }

        match result {
            Ok(value) => nip07_bridge::Nip07BridgeResponse::success(value),
            Err(error) => nip07_bridge::Nip07BridgeResponse::error(error.to_string()),
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
        if self.frame_inbox.take_semantic_index_refresh() {
            self.graph_app.refresh_registry_backed_view_lenses();
        }
    }

    fn apply_pending_workbench_projection_refresh_updates(&mut self) {
        if self.frame_inbox.take_workbench_projection_refresh() {
            let _ =
                persistence_ops::refresh_workbench_projection_from_manifests(&mut self.graph_app);
        }
    }

    fn apply_pending_settings_route_updates(&mut self) {
        for url in self.frame_inbox.take_settings_routes() {
            apply_requested_settings_route_update(&mut self.graph_app, &mut self.tiles_tree, url);
        }
    }

    fn apply_pending_profile_invalidation_updates(&mut self) {
        if self.frame_inbox.take_profile_invalidation() {
            self.graph_app.refresh_registry_backed_view_lenses();
            self.apply_runtime_theme_visuals();
        }
    }

    fn apply_runtime_theme_visuals(&mut self) {
        let resolution = phase3_resolve_active_theme(self.graph_app.default_registry_theme_id());
        let visuals = match resolution.resolved_id.as_str() {
            crate::shell::desktop::runtime::registries::theme::THEME_ID_LIGHT => {
                egui::Visuals::light()
            }
            crate::shell::desktop::runtime::registries::theme::THEME_ID_HIGH_CONTRAST => {
                let mut visuals = egui::Visuals::dark();
                visuals.override_text_color = Some(egui::Color32::WHITE);
                visuals.selection.bg_fill = egui::Color32::from_rgb(255, 230, 0);
                visuals.selection.stroke = egui::Stroke::new(1.5, egui::Color32::BLACK);
                visuals
            }
            _ => egui::Visuals::dark(),
        };
        self.context.egui_context_mut().set_visuals(visuals);
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

    pub(crate) fn set_embedded_content_focus_webview(&mut self, webview_id: Option<WebViewId>) {
        let target = webview_id.map(|renderer_id| {
            crate::shell::desktop::ui::gui_state::EmbeddedContentTarget::WebView {
                renderer_id,
                node_key: self.graph_app.get_node_for_webview(renderer_id),
            }
        });
        focus_state::apply_focus_command(
            &mut self.runtime_state.focus_authority,
            crate::shell::desktop::ui::gui_state::FocusCommand::SetEmbeddedContentFocus { target },
        );
        focus_state::realize_embedded_content_focus_from_authority(
            &self.runtime_state.focus_authority,
            &mut self.graph_app,
        );
    }

    pub(crate) fn focused_embedded_content_webview_id(&self) -> Option<WebViewId> {
        self.runtime_state
            .focus_authority
            .embedded_content_focus
            .as_ref()
            .map(|target| match target {
                crate::shell::desktop::ui::gui_state::EmbeddedContentTarget::WebView {
                    renderer_id,
                    ..
                } => *renderer_id,
            })
            .or_else(|| {
                self.runtime_state
                    .focus_authority
                    .realized_focus_state
                    .as_ref()
                    .and_then(|state| {
                        state.embedded_content_focus.as_ref().map(|target| {
                            match target {
                            crate::shell::desktop::ui::gui_state::EmbeddedContentTarget::WebView {
                                renderer_id,
                                ..
                            } => *renderer_id,
                        }
                        })
                    })
            })
            .or_else(|| self.graph_app.embedded_content_focus_webview())
    }

    pub(crate) fn node_key_for_webview_id(&self, webview_id: WebViewId) -> Option<NodeKey> {
        interaction_queries::node_key_for_webview_id(self, webview_id)
    }

    pub(crate) fn focus_graph_surface(&mut self) {
        clear_embedded_content_focus(&mut self.runtime_state, &mut self.graph_app);
        apply_graph_surface_focus_state(
            &mut self.runtime_state,
            &mut self.graph_app,
            tile_view_ops::active_graph_view_id(&self.tiles_tree),
        );
    }

    pub(crate) fn reclaim_host_focus(&mut self) {
        clear_embedded_content_focus(&mut self.runtime_state, &mut self.graph_app);
        self.surrender_focus();
    }

    pub(crate) fn location_has_focus(&self) -> bool {
        interaction_queries::location_has_focus(self)
    }

    pub(crate) fn request_location_submit(&mut self) {
        interaction_queries::request_location_submit(self)
    }

    pub(crate) fn request_context_command_surface_for_webview(
        &mut self,
        webview_id: WebViewId,
        anchor: [f32; 2],
    ) {
        self.runtime_state
            .pending_webview_context_surface_requests
            .push(
                crate::shell::desktop::ui::gui_state::PendingWebviewContextSurfaceRequest {
                    webview_id,
                    anchor,
                },
            );
    }

    fn persist_active_toolbar_draft(&mut self) {
        let Some(active_pane) = self.runtime_state.focus_authority.pane_activation else {
            return;
        };
        self.runtime_state.toolbar_drafts.insert(
            active_pane,
            ToolbarDraft::from_toolbar_state(&self.toolbar_state),
        );
    }

    fn sync_active_toolbar_draft(&mut self, window: &EmbedderWindow) {
        let next_active_pane = window.focused_pane();
        if self.runtime_state.focus_authority.pane_activation == next_active_pane {
            return;
        }

        self.persist_active_toolbar_draft();
        apply_pane_activation_focus_state(&mut self.runtime_state, next_active_pane);

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

    pub(crate) fn request_toolbar_nav_action_for_webview(
        &mut self,
        webview_id: WebViewId,
        action: ToolbarNavAction,
    ) -> bool {
        let fallback_node = self.graph_app.get_node_for_webview(webview_id);
        toolbar_routing::run_nav_action_for_fallback_node(
            &mut self.graph_app,
            fallback_node,
            action,
        )
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

    pub(crate) fn runtime_focus_state(&self) -> RuntimeFocusState {
        interaction_queries::runtime_focus_state(self)
    }

    pub(crate) fn runtime_focus_inspector(&self) -> RuntimeFocusInspector {
        interaction_queries::runtime_focus_inspector(self)
    }

    /// Update the user interface, but do not paint the updated state.
    pub(crate) fn update(
        &mut self,
        state: &RunningAppState,
        window: &EmbedderWindow,
        headed_window: &headed_window::HeadedWindow,
    ) {
        // Hold the Tokio runtime guard for the entire frame so that any
        // JoinSet::spawn calls (protocol probes, future Nostr relay ops, etc.)
        // made during the render/post-render phases have an active handle.
        let _rt_guard = self.tokio_runtime.enter();
        let _ = self.run_update(GuiUpdateInput {
            state,
            window,
            headed_window,
        });
    }

    fn run_update(&mut self, input: GuiUpdateInput<'_>) -> GuiUpdateOutput {
        self.apply_pending_semantic_index_updates();
        self.apply_pending_workbench_projection_refresh_updates();
        self.apply_pending_settings_route_updates();
        self.apply_pending_profile_invalidation_updates();

        let GuiUpdateInput {
            state,
            window,
            headed_window,
        } = input;
        self.sync_active_toolbar_draft(window);
        // Note: We need Rc<RunningAppState> for runtime viewer creation, but this method
        // is called from trait methods that only provide &RunningAppState.
        // The caller should have Rc available at the call site.
        activate_ui_render_backend(self.rendering_context.as_ref());
        tree_bootstrap::ensure_tiles_tree_root(&mut self.tiles_tree);
        let local_widget_focus = self
            .runtime_state
            .focus_authority
            .local_widget_focus
            .clone();
        focus_state::refresh_realized_runtime_focus_state(
            &mut self.runtime_state.focus_authority,
            &self.graph_app,
            &self.tiles_tree,
            local_widget_focus,
            self.toolbar_state.show_clear_data_confirm,
        );
        debug_assert!(
            self.tiles_tree.root().is_some() || self.tiles_tree.tiles.is_empty(),
            "tile tree root must exist before rendering when the workbench tree is non-empty"
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
            focus_authority,
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            omnibar_search_session,
            toolbar_drafts: _,
            command_palette_toggle_requested,
            pending_webview_context_surface_requests,
            deferred_open_child_webviews,
        } = runtime_state;

        let winit_window = headed_window.winit_window();
        Self::configure_frame_toasts(
            toasts,
            graph_app.workspace.chrome_ui.toast_anchor_preference,
        );
        context.run_ui_frame(winit_window, |ctx, ui_render_backend| {
            Self::execute_update_frame(ExecuteUpdateFrameArgs {
                ctx,
            ui_render_backend,
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
                focus_authority,
                focused_node_hint,
                graph_surface_focused,
                focus_ring_node_key,
                focus_ring_started_at,
                focus_ring_duration,
                omnibar_search_session,
                command_palette_toggle_requested,
                pending_webview_context_surface_requests,
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

    pub(crate) fn handle_clip_extraction_result(
        &mut self,
        result: Result<crate::app::ClipCaptureData, String>,
    ) {
        let capture = match result {
            Ok(capture) => capture,
            Err(error) => {
                self.toasts.error(format!("Clip failed: {error}"));
                return;
            }
        };

        match self.graph_app.create_clip_node_from_capture(&capture) {
            Ok(node_key) => {
                tile_view_ops::open_or_focus_node_pane_with_mode(
                    &mut self.tiles_tree,
                    &self.graph_app,
                    node_key,
                    TileOpenMode::SplitHorizontal,
                );
                self.toasts.success("Created clip node");
            }
            Err(error) => {
                self.toasts.error(format!("Clip failed: {error}"));
            }
        }
    }

    pub(crate) fn handle_clip_batch_extraction_result(
        &mut self,
        result: Result<Vec<crate::app::ClipCaptureData>, String>,
    ) {
        let captures = match result {
            Ok(captures) => captures,
            Err(error) => {
                self.toasts
                    .error(format!("Inspector extraction failed: {error}"));
                return;
            }
        };

        match self.graph_app.open_clip_inspector(captures) {
            Ok(()) => {
                self.toasts.success("Opened page inspector");
            }
            Err(error) => {
                self.toasts
                    .error(format!("Inspector extraction failed: {error}"));
            }
        }
    }

    pub(crate) fn handle_clip_inspector_pointer_result(
        &mut self,
        webview_id: WebViewId,
        result: Result<Vec<crate::app::ClipCaptureData>, String>,
    ) {
        if let Ok(stack) = result {
            self.graph_app
                .update_clip_inspector_pointer_stack(webview_id, stack);
        }
    }

    pub(crate) fn clip_inspector_target_webview_id(&self) -> Option<WebViewId> {
        self.graph_app
            .workspace
            .graph_runtime
            .clip_inspector_state
            .as_ref()
            .map(|state| state.webview_id)
    }

    pub(crate) fn set_zoom_factor(&self, factor: f32) {
        let clamped = accesskit_input::clamp_zoom_factor(factor);
        self.context.egui_context().set_zoom_factor(clamped);
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn diagnostics_state(&self) -> &diagnostics::DiagnosticsState {
        &self.diagnostics_state
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn webview_accessibility_bridge_health_snapshot(
        active_anchor_count: usize,
    ) -> WebViewAccessibilityBridgeHealthSnapshot {
        accessibility::webview_accessibility_bridge_health_snapshot(active_anchor_count)
    }

    pub(crate) fn notify_accessibility_tree_update(
        &mut self,
        webview_id: WebViewId,
        tree_update: servo::accesskit::TreeUpdate,
    ) {
        // Store the most recent update per runtime viewer; it will be injected into
        // egui's accessibility tree at the start of the next frame inside
        // the context.run() callback.
        let replaced_existing = self
            .pending_webview_a11y_updates
            .insert(webview_id, tree_update)
            .is_some();

        #[cfg(feature = "diagnostics")]
        if let Some(tree_update) = self.pending_webview_a11y_updates.get(&webview_id) {
            accessibility::record_webview_a11y_update_queued(
                webview_id,
                tree_update,
                replaced_existing,
                self.pending_webview_a11y_updates.len(),
            );
        }
    }

    pub(crate) fn selected_node_affordance_projection(
        node_key: NodeKey,
    ) -> Option<TileAffordanceAccessibilityProjection> {
        accessibility::selected_node_affordance_projection(node_key)
    }
}
fn ui_overlay_active_from_flags(
    show_command_palette: bool,
    show_context_palette: bool,
    show_help_panel: bool,
    show_scene_overlay: bool,
    show_settings_overlay: bool,
    show_radial_menu: bool,
    show_clip_inspector: bool,
    show_clear_data_confirm: bool,
) -> bool {
    focus_state::ui_overlay_active_from_flags(
        show_command_palette,
        show_context_palette,
        show_help_panel,
        show_scene_overlay,
        show_settings_overlay,
        show_radial_menu,
        show_clip_inspector,
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
