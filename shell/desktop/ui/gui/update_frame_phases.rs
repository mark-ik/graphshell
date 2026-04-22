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
    /// Read-only snapshot (only read: phase 2 never mutates it; the
    /// production mutation path lives on
    /// `crate::shell::desktop::ui::gui::focus_state::apply_canvas_region_focus_state`).
    pub(super) graph_surface_focused: bool,
    pub(super) graph_search: GraphSearchAuthorityMut<'a>,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) toolbar_state: &'a mut ToolbarState,
    pub(super) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
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
    pub(super) control_panel: &'a mut ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub(super) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) graph_tree: &'a mut graph_tree::GraphTree<crate::graph::NodeKey>,
    pub(super) focused_node_hint: Option<NodeKey>,
    pub(super) graph_surface_focused: bool,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) toolbar_state: &'a mut ToolbarState,
    pub(super) clear_data_confirm_deadline_secs: &'a mut Option<f64>,
    pub(super) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) graph_search: GraphSearchAuthorityMut<'a>,
    pub(super) graph_search_output: &'a mut graph_search_flow::GraphSearchFlowOutput,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
}

pub(super) struct SemanticLifecyclePhaseArgs<'a> {
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) graph_tree: &'a mut graph_tree::GraphTree<NodeKey>,
    pub(super) modal_surface_active: bool,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
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
    pub(super) ui_render_backend: &'a mut UiRenderBackendHandle,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) bookmark_import_dialog: &'a mut Option<BookmarkImportDialogState>,
    pub(super) window: &'a EmbedderWindow,
    pub(super) headed_window: &'a headed_window::HeadedWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) graph_tree: &'a mut graph_tree::GraphTree<crate::graph::NodeKey>,
    pub(super) modal_surface_active: bool,
    pub(super) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(super) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    /// Focus mutation bundle assembled by `execute_update_frame` after the
    /// keyboard/toolbar phases have settled `graph_surface_focused`. The
    /// downstream post-render / tile-render path mutates focus fields
    /// through this handle; see
    /// [`FocusAuthorityMut`](crate::shell::desktop::ui::gui_state::FocusAuthorityMut).
    pub(super) focus: crate::shell::desktop::ui::gui_state::FocusAuthorityMut<'a>,
    pub(super) pending_webview_context_surface_requests:
        &'a mut Vec<PendingWebviewContextSurfaceRequest>,
    /// Graph-search mutation bundle. This phase only reads the search
    /// query / matches / active-index / filter-mode for render; `open`
    /// is carried along via the bundle but not consulted.
    pub(super) graph_search: GraphSearchAuthorityMut<'a>,
    /// Command-palette mutation bundle. Carries the `toggle_requested`
    /// signal (read+cleared by pre-frame on the *next* frame; unread
    /// here) and the `CommandPaletteSession` the palette widget reads
    /// and writes during post-render.
    pub(super) command_authority:
        crate::shell::desktop::ui::gui_state::CommandAuthorityMut<'a>,
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
    /// Consolidated tx/rx pair for async thumbnail capture result
    /// delivery. See [`ThumbnailChannel`](crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailChannel).
    pub(super) thumbnail_channel: &'a super::super::thumbnail_pipeline::ThumbnailChannel,
    pub(super) thumbnail_capture_in_flight: &'a mut HashSet<WebViewId>,
    pub(super) command_authority:
        crate::shell::desktop::ui::gui_state::CommandAuthorityMut<'a>,
    pub(super) control_panel: &'a mut ControlPanel,
}

pub(super) struct ExecuteUpdateFrameArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) ui_render_backend: &'a mut UiRenderBackendHandle,
    pub(super) winit_window: &'a Window,
    pub(super) state: &'a RunningAppState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) headed_window: &'a headed_window::HeadedWindow,
    pub(super) graph_app: &'a mut GraphBrowserApp,
    pub(super) bookmark_import_dialog: &'a mut Option<BookmarkImportDialogState>,
    pub(super) pending_webview_a11y_updates: &'a mut HashMap<WebViewId, accesskit::TreeUpdate>,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) graph_tree: &'a mut graph_tree::GraphTree<crate::graph::NodeKey>,
    pub(super) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(super) toolbar_state: &'a mut ToolbarState,
    pub(super) clear_data_confirm_deadline_secs: &'a mut Option<f64>,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) clipboard: &'a mut Option<Clipboard>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) thumbnail_channel: &'a super::super::thumbnail_pipeline::ThumbnailChannel,
    pub(super) thumbnail_capture_in_flight: &'a mut HashSet<WebViewId>,
    pub(super) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) graph_search: GraphSearchAuthorityMut<'a>,
    pub(super) focus_authority: &'a mut RuntimeFocusAuthorityState,
    pub(super) focused_node_hint: &'a mut Option<NodeKey>,
    /// Read-only snapshot; production mutation paths live on
    /// `GraphshellRuntime` (see `apply_canvas_region_focus_state`).
    pub(super) graph_surface_focused: bool,
    pub(super) focus_ring_node_key: &'a mut Option<NodeKey>,
    pub(super) focus_ring_started_at: &'a mut Option<std::time::Instant>,
    /// Read-only; focus-ring animation length is owned by runtime state
    /// and sourced from `chrome_ui.focus_ring_settings`. Never mutated
    /// by any phase.
    pub(super) focus_ring_duration: Duration,
    pub(super) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub(super) command_authority:
        crate::shell::desktop::ui::gui_state::CommandAuthorityMut<'a>,
    pub(super) pending_webview_context_surface_requests:
        &'a mut Vec<PendingWebviewContextSurfaceRequest>,
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
