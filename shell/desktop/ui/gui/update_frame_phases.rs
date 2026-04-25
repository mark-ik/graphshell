/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use graphshell_runtime::FrameViewModel;

use super::*;

pub(super) struct GraphSearchAndKeyboardPhaseArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) window: &'a EmbedderWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
    /// Lane B' (2026-04-23): runtime-owned state previously threaded as
    /// individual `graph_app` / `graph_surface_focused` / `graph_search`
    /// (bundle) / `focus_authority` / `toolbar_state` / `viewer_surfaces` /
    /// `viewer_surface_host` / `webview_creation_backpressure` fields now
    /// flows through this single ref. Phase function destructures
    /// internally.
    pub(super) runtime: &'a mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
}

pub(super) struct ToolbarAndGraphSearchWindowPhaseArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) root_ui: &'a mut egui::Ui,
    pub(super) winit_window: &'a Window,
    pub(super) state: &'a RunningAppState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) graph_search_output: &'a mut graph_search_flow::GraphSearchFlowOutput,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
    /// Lane B' (2026-04-23): runtime-owned state previously threaded as
    /// 15 individual field refs (graph_app / control_panel / graph_tree /
    /// focus_authority / toolbar_state / clear_data_confirm_deadline_secs /
    /// omnibar_search_session / omnibar_provider_suggestion_driver /
    /// command_surface_telemetry / viewer_surfaces / viewer_surface_host /
    /// webview_creation_backpressure plus the graph_search bundle and the
    /// read-only `focused_node_hint` / `graph_surface_focused`) now flows
    /// through this single ref. Phase function destructures internally.
    pub(super) runtime: &'a mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
}

pub(super) struct SemanticLifecyclePhaseArgs<'a> {
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) modal_surface_active: bool,
    pub(super) window: &'a EmbedderWindow,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
    /// Lane B' (2026-04-23): runtime-owned state previously threaded as
    /// `graph_app` / `graph_tree` / `focus_authority` / `viewer_surfaces` /
    /// `viewer_surface_host` / `webview_creation_backpressure` /
    /// `command_surface_telemetry` (7 individual refs) now flows through
    /// this single ref. Phase function destructures internally.
    pub(super) runtime: &'a mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
}

pub(super) struct SemanticAndPostRenderPhaseArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) root_ui: &'a mut egui::Ui,
    pub(super) ui_render_backend: &'a mut UiRenderBackendHandle,
    pub(super) window: &'a EmbedderWindow,
    pub(super) headed_window: &'a headed_window::HeadedWindow,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) modal_surface_active: bool,
    pub(super) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) responsive_webviews: &'a HashSet<WebViewId>,
    pub(super) open_node_tile_after_intents: &'a mut Option<TileOpenMode>,
    pub(super) frame_intents: &'a mut Vec<GraphIntent>,
    /// Lane B' (2026-04-23): runtime-owned state previously threaded as
    /// `graph_app` / `bookmark_import_dialog` / `graph_tree` /
    /// `viewer_surfaces` / `viewer_surface_host` /
    /// `webview_creation_backpressure` / `focus_authority` / the `focus`
    /// FocusAuthorityMut bundle / `pending_webview_context_surface_requests`
    /// / the `graph_search` GraphSearchAuthorityMut bundle / the
    /// `command_authority` CommandAuthorityMut bundle / `registry_runtime`
    /// / `control_panel` / `command_surface_telemetry` (15 individual
    /// refs plus 3 bundles wrapping 11 more fields) now flows through this
    /// single ref. The phase function destructures internally and assembles
    /// the bundles for the deeper sub-phases (post-render, semantic
    /// lifecycle).
    pub(super) runtime: &'a mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
    /// §12.6 (2026-04-24): previous-frame view-model; forwarded to
    /// post-render so tile_render_pass can consume pre-projected state
    /// instead of reading runtime fields directly.
    pub(super) cached_view_model: Option<&'a FrameViewModel>,
}

pub(super) struct PreFrameAndIntentInitArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) state: &'a RunningAppState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    /// Consolidated tx/rx pair for async thumbnail capture result
    /// delivery. See [`ThumbnailChannel`](crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailChannel).
    pub(super) thumbnail_channel: &'a super::super::thumbnail_pipeline::ThumbnailChannel,
    /// Lane B' (2026-04-23): runtime-owned state previously threaded as
    /// individual `graph_app` / `thumbnail_capture_in_flight` /
    /// `command_authority` / `control_panel` fields now flows through
    /// this single ref. Phase function destructures internally.
    pub(super) runtime: &'a mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
}

/// Top-level frame-phase arguments.
///
/// Lane B' (2026-04-23): Runtime-owned state previously threaded through
/// this struct as ~23 individual `&mut` field references — `graph_app`,
/// `graph_tree`, `toolbar_state`, the `graph_search_*` quintet (via
/// `GraphSearchAuthorityMut`), the focus quartet (`focused_node_hint`,
/// `focus_ring_*`, `graph_surface_focused`, `focus_authority`), the
/// command-palette pair (via `CommandAuthorityMut`), and so on — is now
/// delivered as a single [`GraphshellRuntime`] reference. The destructure
/// that previously decomposed runtime at `gui.rs:920` has moved inside
/// `execute_update_frame` so individual bindings remain scoped to the
/// phase-pipeline body that needs them. Only host-side / non-runtime
/// state remains as individual fields.
pub(super) struct ExecuteUpdateFrameArgs<'a> {
    pub(super) ctx: &'a egui::Context,
    pub(super) root_ui: &'a mut egui::Ui,
    pub(super) ui_render_backend: &'a mut UiRenderBackendHandle,
    pub(super) winit_window: &'a Window,
    pub(super) state: &'a RunningAppState,
    pub(super) window: &'a EmbedderWindow,
    pub(super) headed_window: &'a headed_window::HeadedWindow,
    pub(super) pending_webview_a11y_updates: &'a mut HashMap<WebViewId, accesskit::TreeUpdate>,
    pub(super) tiles_tree: &'a mut Tree<TileKind>,
    pub(super) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(super) toasts: &'a mut egui_notify::Toasts,
    pub(super) clipboard: &'a mut Option<Clipboard>,
    pub(super) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(super) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(super) thumbnail_channel: &'a super::super::thumbnail_pipeline::ThumbnailChannel,
    pub(super) app_state: &'a Option<Rc<RunningAppState>>,
    pub(super) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(super) window_rendering_context: &'a Rc<WindowRenderingContext>,
    /// Single mutable reference to the runtime. `execute_update_frame`
    /// destructures this internally to obtain the per-field bindings each
    /// phase needs. `diagnostics_state` (formerly a separate field, §12.16
    /// 2026-04-24) now lives on the runtime; phases reach for it via
    /// `runtime.diagnostics_state` directly.
    pub(super) runtime: &'a mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
    /// §12.6 (2026-04-24): the FrameViewModel produced by the previous
    /// frame's tick(). `None` on the very first frame. Downstream
    /// render sites (tile_render_pass focus-ring alpha) consume this
    /// instead of re-reading runtime fields directly.
    pub(super) cached_view_model: Option<&'a FrameViewModel>,
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
