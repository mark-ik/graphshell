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

use super::graph_search_flow::{self, GraphSearchFlowArgs};
use super::graph_search_ui::{self, GraphSearchUiArgs};
use super::gui_frame::{self, PreFrameIngestArgs, ToolbarDialogPhaseArgs};
use super::lifecycle_intents;
use super::persistence_ops;
#[cfg(test)]
use super::semantic_event_pipeline;
use super::thumbnail_pipeline::ThumbnailCaptureResult;
#[cfg(test)]
use super::thumbnail_pipeline;
use super::tile_compositor;
#[cfg(test)]
use super::tile_grouping;
#[cfg(test)]
use super::tile_invariants;
use super::tile_kind::TileKind;
use super::tile_runtime;
use super::tile_view_ops::{self, TileOpenMode, ToggleTileViewArgs};
use super::toolbar_routing::ToolbarOpenMode;
use super::toolbar_ui::OmnibarSearchSession;
use super::webview_backpressure::WebviewCreationBackpressureState;
use super::webview_status_sync;
use crate::app::{
    ClipboardCopyKind, ClipboardCopyRequest, GraphBrowserApp, GraphIntent, GraphViewId, LifecycleCause,
    PendingTileOpenMode, SearchDisplayMode, ToastAnchorPreference,
};
use crate::desktop::event_loop::AppEvent;
use crate::desktop::headed_window;
use crate::graph::NodeKey;
use crate::running_app_state::RunningAppState;
use crate::search::fuzzy_match_node_keys;
use crate::window::EmbedderWindow;
#[cfg(test)]
use crate::window::GraphSemanticEvent;

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

    /// Count of webview accessibility tree updates that could not be bridged.
    webview_accessibility_updates_dropped: u64,

    /// Whether we've already warned about dropped webview accessibility updates.
    webview_accessibility_warned: bool,

    /// Cached reference to RunningAppState for webview creation
    state: Option<Rc<RunningAppState>>,

    /// Runtime UI state used by the frame coordinator and toolbar/search flows.
    runtime_state: GuiRuntimeState,
}

impl Drop for Gui {
    fn drop(&mut self) {
        if let Ok(layout_json) = serde_json::to_string(&self.tiles_tree) {
            self.graph_app.save_tile_layout_json(&layout_json);
        } else {
            warn!("Failed to serialize tile layout for persistence");
        }
        self.graph_app.take_snapshot();
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
        *tiles_tree = restored_tree;
        return true;
    }

    if let Some(layout_json) = graph_app.load_tile_layout_json()
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_webview_tile_keys_only(&mut restored_tree, graph_app);
        if restored_tree.root().is_some() {
            graph_app.mark_session_workspace_layout_json(&layout_json);
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
            graph_data_dir.unwrap_or_else(crate::persistence::GraphStore::default_data_dir);
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
            matches!(graph_app.search_display_mode, SearchDisplayMode::Filter);

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
                .with_anchor(Self::toast_anchor(graph_app.toast_anchor_preference))
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
            webview_accessibility_updates_dropped: 0,
            webview_accessibility_warned: false,
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
        !self.has_active_webview_tile()
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
        if !self.has_active_webview_tile() {
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
            let Some(Tile::Pane(TileKind::WebView(node_key))) = self.tiles_tree.tiles.get(tile_id)
            else {
                continue;
            };
            let Some(rect) = self.tiles_tree.tiles.rect(tile_id) else {
                continue;
            };
            if !rect.contains(cursor) {
                continue;
            }
            let Some(webview_id) = self.graph_app.get_webview_for_node(*node_key) else {
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
        tile_compositor::focused_webview_id_for_tree(
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
        tile_compositor::focused_webview_id_for_tree(&self.tiles_tree, &self.graph_app, None)
    }

    pub(crate) fn set_focused_webview_id(&mut self, webview_id: WebViewId) {
        self.runtime_state.focused_webview_hint = Some(webview_id);
        self.runtime_state.graph_surface_focused = false;
    }

    pub(crate) fn focus_graph_surface(&mut self) {
        self.runtime_state.focused_webview_hint = None;
        self.runtime_state.graph_surface_focused = true;
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
        self.graph_app.show_command_palette
            || self.graph_app.show_help_panel
            || self.graph_app.show_physics_panel
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
            state: app_state,
            runtime_state,
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
            .with_anchor(Self::toast_anchor(graph_app.toast_anchor_preference))
            .with_margin(egui::vec2(12.0, 12.0));
        context.run(winit_window, |ctx| {
            graph_app.tick_frame();
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

            // Phase 1: apply semantic/UI intents before lifecycle reconciliation.
            gui_frame::apply_intents_if_any(graph_app, tiles_tree, &mut frame_intents);
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
                    tile_view_ops::open_or_focus_webview_tile_with_mode(
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
        let graph_search_available = Self::active_webview_tile_node(tiles_tree).is_none();
        graph_app.search_display_mode = if *graph_search_filter_mode {
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
            let Some(node) = graph_app.graph.get_node(key) else {
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

    fn handle_pending_open_node_after_intents(
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &mut Tree<TileKind>,
        open_node_tile_after_intents: &mut Option<TileOpenMode>,
        frame_intents: &mut Vec<GraphIntent>,
    ) {
        if let Some(open_request) = graph_app.take_pending_open_node_request() {
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
                gui_frame::active_webview_tile_node(tiles_tree)
            } else {
                None
            };
            let node_already_in_workspace = tiles_tree.tiles.iter().any(|(_, tile)| {
                matches!(
                    tile,
                    Tile::Pane(TileKind::WebView(existing_key)) if *existing_key == node_key
                )
            });
            tile_view_ops::open_or_focus_webview_tile_with_mode(tiles_tree, node_key, open_mode);
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

    fn has_active_webview_tile(&self) -> bool {
        self.tiles_tree.active_tiles().into_iter().any(|tile_id| {
            matches!(
                self.tiles_tree.tiles.get(tile_id),
                Some(Tile::Pane(TileKind::WebView(_)))
            )
        })
    }

    fn active_webview_tile_node(tiles_tree: &Tree<TileKind>) -> Option<crate::graph::NodeKey> {
        tiles_tree.active_tiles().into_iter().find_map(|tile_id| {
            match tiles_tree.tiles.get(tile_id) {
                Some(Tile::Pane(TileKind::WebView(node_key))) => Some(*node_key),
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
        let has_webview_tiles = tile_runtime::has_any_webview_tiles(&self.tiles_tree);
        let selected_node_url = self.graph_app.get_single_selected_node().and_then(|key| {
            self.graph_app
                .graph
                .get_node(key)
                .map(|node| node.url.clone())
        });
        let focused_webview_id = self.focused_webview_id();
        webview_status_sync::update_location_in_toolbar(
            self.toolbar_state.location_dirty,
            &mut self.toolbar_state.location,
            has_webview_tiles,
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

    pub(crate) fn notify_accessibility_tree_update(
        &mut self,
        webview_id: WebViewId,
        _tree_update: accesskit::TreeUpdate,
    ) {
        self.webview_accessibility_updates_dropped += 1;
        if !self.webview_accessibility_warned {
            self.webview_accessibility_warned = true;
            warn!(
                "WebView accessibility update dropped for {:?}: no embedder bridge available yet (issue #41930)",
                webview_id
            );
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

    *matches = fuzzy_match_node_keys(&graph_app.graph, query);
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
fn graph_intent_for_thumbnail_result(
    graph_app: &GraphBrowserApp,
    result: &ThumbnailCaptureResult,
) -> Option<GraphIntent> {
    thumbnail_pipeline::graph_intent_for_thumbnail_result(graph_app, result)
}
