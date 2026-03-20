/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::ui::gui_state::PendingWebviewContextSurfaceRequest;

pub(super) struct GraphSearchAndKeyboardPhaseArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) window: &'a EmbedderWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) graph_search_open: &'a mut bool,
    pub(super) graph_search_query: &'a mut String,
    pub(super) graph_search_filter_mode: &'a mut bool,
    pub(super) graph_search_matches: &'a mut Vec<NodeKey>,
    pub(super) graph_search_active_match_index: &'a mut Option<usize>,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) toolbar_state: &'a mut ToolbarState,
    pub(super) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
}

pub(super) struct ToolbarAndGraphSearchWindowPhaseArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) winit_window: &'a Window,
    pub(super) state: &'a RunningAppState,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    #[cfg(feature = "diagnostics")]
    pub(super) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) focused_node_hint: Option<NodeKey>,
    pub(super) graph_surface_focused: bool,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) toolbar_state: &'a mut ToolbarState,
    pub(super) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) graph_search_open: &'a mut bool,
    pub(super) graph_search_query: &'a mut String,
    pub(super) graph_search_filter_mode: &'a mut bool,
    pub(super) graph_search_matches: &'a mut Vec<NodeKey>,
    pub(super) graph_search_active_match_index: &'a mut Option<usize>,
    pub(super) graph_search_output: &'a mut graph_search_flow::GraphSearchFlowOutput,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
}

pub(super) struct SemanticLifecyclePhaseArgs<'a> {
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) modal_surface_active: bool,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
}

pub(super) struct SemanticAndPostRenderPhaseArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) window: &'a EmbedderWindow,
    pub(super) headed_window: &'a headed_window::HeadedWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) modal_surface_active: bool,
    pub(super) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(super) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) focused_node_hint: &'a mut Option<NodeKey>,
    pub(super) graph_surface_focused: &'a mut bool,
    pub(super) focus_ring_node_key: &'a mut Option<NodeKey>,
    pub(super) focus_ring_started_at: &'a mut Option<std::time::Instant>,
    pub(super) focus_ring_duration: &'a mut Duration,
    pub(super) pending_webview_context_surface_requests:
        &'a mut Vec<PendingWebviewContextSurfaceRequest>,
    pub(super) graph_search_query: &'a mut String,
    pub(super) graph_search_matches: &'a mut Vec<NodeKey>,
    pub(super) graph_search_active_match_index: &'a mut Option<usize>,
    pub(super) graph_search_filter_mode: &'a mut bool,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) registry_runtime: &'a RegistryRuntime,
    pub(super) control_panel: &'a mut ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub(super) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
}

pub(super) struct PreFrameAndIntentInitArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) state: &'a RunningAppState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) thumbnail_capture_tx: &'a Sender<ThumbnailCaptureResult>,
    pub(super) thumbnail_capture_rx: &'a Receiver<ThumbnailCaptureResult>,
    pub(super) thumbnail_capture_in_flight: &'a mut HashSet<WebViewId>,
    pub(super) command_palette_toggle_requested: &'a mut bool,
    pub(super) control_panel: &'a mut ControlPanel,
}

pub(super) struct ExecuteUpdateFrameArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) winit_window: &'a Window,
    pub(super) state: &'a RunningAppState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) headed_window: &'a headed_window::HeadedWindow,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) pending_webview_a11y_updates: &'a mut HashMap<WebViewId, accesskit::TreeUpdate>,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(super) toolbar_state: &'a mut ToolbarState,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) clipboard: &'a mut Option<Clipboard>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) thumbnail_capture_tx: &'a Sender<ThumbnailCaptureResult>,
    pub(super) thumbnail_capture_rx: &'a Receiver<ThumbnailCaptureResult>,
    pub(super) thumbnail_capture_in_flight: &'a mut HashSet<WebViewId>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) graph_search_open: &'a mut bool,
    pub(super) graph_search_query: &'a mut String,
    pub(super) graph_search_filter_mode: &'a mut bool,
    pub(super) graph_search_matches: &'a mut Vec<NodeKey>,
    pub(super) graph_search_active_match_index: &'a mut Option<usize>,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) focused_node_hint: &'a mut Option<NodeKey>,
    pub(super) graph_surface_focused: &'a mut bool,
    pub(super) focus_ring_node_key: &'a mut Option<NodeKey>,
    pub(super) focus_ring_started_at: &'a mut Option<std::time::Instant>,
    pub(super) focus_ring_duration: &'a mut Duration,
    pub(super) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub(super) command_palette_toggle_requested: &'a mut bool,
    pub(super) pending_webview_context_surface_requests:
        &'a mut Vec<PendingWebviewContextSurfaceRequest>,
    pub(super) deferred_open_child_webviews: &'a mut Vec<WebViewId>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) registry_runtime: &'a RegistryRuntime,
    pub(super) control_panel: &'a mut ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub(super) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum UpdateFrameStage {
    Prelude,
    PreFrameInit,
    GraphSearchAndKeyboard,
    ToolbarAndGraphSearchWindow,
    SemanticAndPostRender,
    Finalize,
}

pub(super) const UPDATE_FRAME_STAGE_SEQUENCE: [UpdateFrameStage; 6] = [
    UpdateFrameStage::Prelude,
    UpdateFrameStage::PreFrameInit,
    UpdateFrameStage::GraphSearchAndKeyboard,
    UpdateFrameStage::ToolbarAndGraphSearchWindow,
    UpdateFrameStage::SemanticAndPostRender,
    UpdateFrameStage::Finalize,
];
