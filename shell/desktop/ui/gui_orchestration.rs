/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use arboard::Clipboard;

use crate::app::{
    ClipboardCopyKind, ClipboardCopyRequest, GraphBrowserApp, GraphIntent, LifecycleCause,
    PendingTileOpenMode, SearchDisplayMode, UndoBoundaryReason, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::services::search::fuzzy_match_node_keys;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_CLIPBOARD_COPY_FAILED, CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_DISPATCH_CONSUMED,
    CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED, CHANNEL_UX_DISPATCH_PHASE, CHANNEL_UX_DISPATCH_STARTED,
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_NAVIGATION_VIOLATION,
};
use crate::shell::desktop::ui::graph_search_flow::{self, GraphSearchFlowArgs};
use crate::shell::desktop::ui::graph_search_ui::{self, GraphSearchUiArgs};
use crate::shell::desktop::ui::gui_frame::ToolbarDialogPhaseArgs;
use crate::shell::desktop::ui::gui_frame::{self, PreFrameIngestArgs};
use crate::shell::desktop::ui::nav_targeting;
use crate::shell::desktop::ui::gui_state::ToolbarState;
use crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailCaptureResult;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{TileOpenMode, ToggleTileViewArgs};
use egui_tiles::Tree;
use servo::WebViewId;
use servo::{OffscreenRenderingContext, WindowRenderingContext};
use std::rc::Rc;
use winit::window::Window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UxEventKind {
    PointerDown,
    PointerUp,
    PointerMove,
    PointerEnter,
    PointerLeave,
    KeyDown,
    KeyUp,
    Scroll,
    PinchZoom,
    FocusIn,
    FocusOut,
    Focus,
    Blur,
    Action,
    UxBridgeCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UxDispatchPhase {
    Capture = 1,
    Target = 2,
    Bubble = 3,
    Default = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct UxDispatchControl {
    stop_propagation: bool,
    stop_immediate_propagation: bool,
    prevent_default: bool,
}

#[derive(Clone, Debug)]
struct UxDispatchPath {
    nodes: Vec<u64>,
}

impl UxDispatchPath {
    fn is_valid(&self) -> bool {
        if self.nodes.len() < 2 {
            return false;
        }
        if self.nodes.first().copied() != Some(0) {
            return false;
        }
        let mut seen = HashSet::new();
        self.nodes.iter().all(|node| seen.insert(*node))
    }
}

const UX_DISPATCH_NODE_ROOT: u64 = 0;
const UX_DISPATCH_NODE_WORKBENCH: u64 = 1;
const UX_DISPATCH_NODE_COMMAND_SURFACE: u64 = 2;
const UX_DISPATCH_NODE_TOOL_SURFACE: u64 = 3;
const UX_DISPATCH_NODE_GRAPH_SURFACE: u64 = 4;

pub(crate) struct PreFramePhaseOutput {
    pub(crate) frame_intents: Vec<GraphIntent>,
    pub(crate) responsive_webviews: HashSet<WebViewId>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_pre_frame_phase(
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
        graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleCommandPalette);
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
        responsive_webviews: pre_frame.responsive_webviews,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_graph_search_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    graph_search_open: &mut bool,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    toolbar_state: &mut ToolbarState,
    frame_intents: &mut Vec<GraphIntent>,
    has_active_node_pane: bool,
) -> graph_search_flow::GraphSearchFlowOutput {
    let graph_search_available = !has_active_node_pane;
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
pub(crate) fn run_graph_search_window_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    graph_search_output: &mut graph_search_flow::GraphSearchFlowOutput,
) {
    if should_render_graph_search_window(toolbar_visible, graph_search_open, is_graph_view) {
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
}

fn should_render_graph_search_window(
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
) -> bool {
    toolbar_visible && graph_search_open && is_graph_view
}

pub(crate) fn active_graph_search_match(
    matches: &[NodeKey],
    active_index: Option<usize>,
) -> Option<NodeKey> {
    let idx = active_index?;
    matches.get(idx).copied()
}

fn refresh_graph_search_matches(
    graph_app: &GraphBrowserApp,
    query: &str,
    matches: &mut Vec<NodeKey>,
    active_index: &mut Option<usize>,
) {
    if clear_graph_search_matches_if_query_empty(query, matches, active_index) {
        return;
    }

    *matches = fuzzy_match_node_keys(graph_app.domain_graph(), query);
    sync_graph_search_active_index(matches, active_index);
}

fn clear_graph_search_matches_if_query_empty(
    query: &str,
    matches: &mut Vec<NodeKey>,
    active_index: &mut Option<usize>,
) -> bool {
    if query.trim().is_empty() {
        matches.clear();
        *active_index = None;
        return true;
    }

    false
}

fn sync_graph_search_active_index(matches: &[NodeKey], active_index: &mut Option<usize>) {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_keyboard_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    suppress_toggle_view: bool,
    frame_intents: &mut Vec<GraphIntent>,
) {
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
            responsive_webviews,
            webview_creation_backpressure,
            suppress_toggle_view,
        },
        frame_intents,
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
            crate::shell::desktop::workbench::tile_view_ops::toggle_tile_view(ToggleTileViewArgs {
                tiles_tree,
                graph_app,
                window,
                app_state,
                base_rendering_context: rendering_context,
                window_rendering_context,
                tile_rendering_contexts,
                responsive_webviews,
                webview_creation_backpressure,
                lifecycle_intents: frame_intents,
            });
        },
        |tiles_tree, tile_rendering_contexts, tile_favicon_textures, favicon_textures| {
            crate::shell::desktop::workbench::tile_runtime::reset_runtime_webview_state(
                tiles_tree,
                tile_rendering_contexts,
                tile_favicon_textures,
                favicon_textures,
            );
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_toolbar_phase(
    ctx: &egui::Context,
    winit_window: &Window,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    #[cfg(feature = "diagnostics")] diagnostics_state: &mut diagnostics::DiagnosticsState,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    focused_node_hint: Option<NodeKey>,
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
            active_toolbar_pane: window.focused_pane(),
            focused_node_hint,
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
    handle_toolbar_toggle_tile_view_request(
        toolbar_output.toggle_tile_view_requested,
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
    handle_toolbar_open_selected_mode_after_submit(
        toolbar_output.open_selected_mode_after_submit,
        open_node_tile_after_intents,
    );

    (toolbar_output.toolbar_visible, is_graph_view)
}

#[allow(clippy::too_many_arguments)]
fn handle_toolbar_toggle_tile_view_request(
    toggle_tile_view_requested: bool,
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    if toggle_tile_view_requested && !graph_app.history_health_summary().preview_mode_active {
        crate::shell::desktop::workbench::tile_view_ops::toggle_tile_view(ToggleTileViewArgs {
            tiles_tree,
            graph_app,
            window,
            app_state,
            base_rendering_context: rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            responsive_webviews,
            webview_creation_backpressure,
            lifecycle_intents: frame_intents,
        });
    }
}

fn handle_toolbar_open_selected_mode_after_submit(
    open_selected_mode_after_submit: Option<ToolbarOpenMode>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
) {
    if let Some(open_mode) = open_selected_mode_after_submit {
        *open_node_tile_after_intents = Some(open_mode_from_toolbar(open_mode));
    }
}

pub(crate) fn handle_pending_clipboard_copy_requests(
    graph_app: &mut GraphBrowserApp,
    clipboard: &mut Option<Clipboard>,
    toasts: &mut egui_notify::Toasts,
) {
    while let Some(ClipboardCopyRequest { key, kind }) = graph_app.take_pending_clipboard_copy() {
        handle_pending_clipboard_copy_request(graph_app, clipboard, toasts, key, kind);
    }
}

fn handle_pending_clipboard_copy_request(
    graph_app: &GraphBrowserApp,
    clipboard: &mut Option<Clipboard>,
    toasts: &mut egui_notify::Toasts,
    key: NodeKey,
    kind: ClipboardCopyKind,
) {
    let Some(value) = clipboard_copy_value_for_node(graph_app, key, kind, toasts) else {
        return;
    };

    ensure_clipboard_initialized(clipboard);
    let Some(cb) = clipboard.as_mut() else {
        emit_clipboard_copy_failure("clipboard unavailable".len());
        toasts.error(CLIPBOARD_STATUS_UNAVAILABLE_TEXT);
        return;
    };

    match cb.set_text(value) {
        Ok(()) => emit_clipboard_copy_success_toast(toasts, kind),
        Err(e) => {
            emit_clipboard_copy_failure(e.to_string().len());
            toasts.error(clipboard_copy_failure_text(e.to_string().as_str()));
        }
    }
}

const CLIPBOARD_STATUS_SUCCESS_URL_TEXT: &str = "Copied URL";
const CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT: &str = "Copied title";
const CLIPBOARD_STATUS_UNAVAILABLE_TEXT: &str = "Clipboard unavailable";
const CLIPBOARD_STATUS_EMPTY_TEXT: &str = "Nothing to copy";
const CLIPBOARD_STATUS_FAILURE_PREFIX: &str = "Copy failed";
const CLIPBOARD_STATUS_MISSING_NODE_SUGGESTION_TEXT: &str = "select a node and try again";

fn clipboard_copy_value_for_node(
    graph_app: &GraphBrowserApp,
    key: NodeKey,
    kind: ClipboardCopyKind,
    toasts: &mut egui_notify::Toasts,
) -> Option<String> {
    let Some(node) = graph_app.domain_graph().get_node(key) else {
        toasts.error(clipboard_copy_missing_node_failure_text());
        return None;
    };

    let value = match kind {
        ClipboardCopyKind::Url => node.url.clone(),
        ClipboardCopyKind::Title => clipboard_title_or_url(node.title.as_str(), node.url.as_str()),
    };

    if value.trim().is_empty() {
        toasts.warning(CLIPBOARD_STATUS_EMPTY_TEXT);
        return None;
    }

    Some(value)
}

fn clipboard_title_or_url(title: &str, url: &str) -> String {
    if title.is_empty() {
        url.to_owned()
    } else {
        title.to_owned()
    }
}

fn ensure_clipboard_initialized(clipboard: &mut Option<Clipboard>) -> bool {
    if clipboard.is_none() {
        *clipboard = Clipboard::new().ok();
    }
    clipboard.is_some()
}

fn emit_clipboard_copy_failure(byte_len: usize) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_CLIPBOARD_COPY_FAILED,
        byte_len,
    });
}

fn emit_clipboard_copy_success_toast(toasts: &mut egui_notify::Toasts, kind: ClipboardCopyKind) {
    toasts.success(clipboard_copy_success_text(kind));
}

fn clipboard_copy_success_text(kind: ClipboardCopyKind) -> &'static str {
    match kind {
        ClipboardCopyKind::Url => CLIPBOARD_STATUS_SUCCESS_URL_TEXT,
        ClipboardCopyKind::Title => CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT,
    }
}

fn clipboard_copy_failure_text(detail: &str) -> String {
    format!("{CLIPBOARD_STATUS_FAILURE_PREFIX}: {detail}")
}

fn clipboard_copy_missing_node_failure_text() -> String {
    clipboard_copy_failure_text(
        format!("node no longer exists; {CLIPBOARD_STATUS_MISSING_NODE_SUGGESTION_TEXT}").as_str(),
    )
}

pub(crate) fn handle_pending_open_node_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let queued_open_mode = open_node_tile_after_intents.take();
    let pending_open_request = take_pending_open_node_request_selection(graph_app);

    log::debug!(
        "gui: pending open node phase queued_mode={:?} pending_request={:?} selected={:?}",
        queued_open_mode,
        pending_open_request,
        graph_app.get_single_selected_node()
    );

    let open_candidate = pending_open_request
        .map(|(node_key, mode)| (Some(node_key), mode))
        .or_else(|| queued_open_mode.map(|mode| (None, mode)));

    if let Some((request_node_key, open_mode)) = open_candidate
        && let Some(node_key) = request_node_key.or_else(|| graph_app.get_single_selected_node())
    {
        if request_node_key.is_some() {
            frame_intents.push(GraphIntent::SelectNode {
                key: node_key,
                multi_select: false,
            });
        }
        execute_pending_open_node_after_intents(
            graph_app,
            tiles_tree,
            frame_intents,
            node_key,
            open_mode,
        );

        log::debug!(
            "gui: executed pending open node {:?} mode {:?}; active_tiles={}",
            node_key,
            open_mode,
            tiles_tree.active_tiles().len()
        );
    } else if open_candidate.is_some() {
        log::debug!("gui: pending open node skipped because no valid selected node is available");
    }
}

pub(crate) fn handle_pending_open_note_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    let Some(note_id) = graph_app.take_pending_open_note_request() else {
        return;
    };

    let linked_node = graph_app
        .note_record(note_id)
        .and_then(|note| note.linked_node);
    if let Some(node_key) = linked_node
        && graph_app.domain_graph().get_node(node_key).is_some()
    {
        crate::shell::desktop::workbench::tile_view_ops::open_or_focus_node_pane(
            tiles_tree, graph_app, node_key,
        );
    }

    crate::shell::desktop::workbench::tile_view_ops::open_or_focus_tool_pane(
        tiles_tree,
        ToolPaneState::HistoryManager,
    );
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
}

pub(crate) fn handle_pending_open_clip_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    let Some(_clip_id) = graph_app.take_pending_open_clip_request() else {
        return;
    };

    crate::shell::desktop::workbench::tile_view_ops::open_or_focus_tool_pane(
        tiles_tree,
        ToolPaneState::HistoryManager,
    );
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
}

fn take_pending_open_node_request_selection(
    graph_app: &mut GraphBrowserApp,
) -> Option<(NodeKey, TileOpenMode)> {
    if let Some(open_request) = graph_app.take_pending_open_node_request() {
        log::debug!(
            "gui: handle_pending_open_node_after_intents taking request for {:?}",
            open_request.key
        );
        let open_mode = open_mode_from_pending(open_request.mode);
        return Some((open_request.key, open_mode));
    }

    None
}

fn execute_pending_open_node_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    frame_intents: &mut Vec<GraphIntent>,
    node_key: NodeKey,
    open_mode: TileOpenMode,
) {
    capture_open_node_undo_checkpoint(graph_app, tiles_tree);
    let anchor_before_open = anchor_before_tab_open(tiles_tree, open_mode);
    let node_already_in_workspace = is_node_already_in_workspace(tiles_tree, node_key);
    log::debug!(
        "gui: calling open_or_focus_node_pane_with_mode for {:?} mode {:?}",
        node_key,
        open_mode
    );
    crate::shell::desktop::workbench::tile_view_ops::open_or_focus_node_pane_with_mode(
        tiles_tree, graph_app, node_key, open_mode,
    );
    maybe_push_grouped_edge_after_tab_open(
        frame_intents,
        open_mode,
        node_already_in_workspace,
        anchor_before_open,
        node_key,
    );
    frame_intents.push(
        lifecycle_intents::promote_node_to_active(node_key, LifecycleCause::UserSelect).into(),
    );
}

fn capture_open_node_undo_checkpoint(graph_app: &mut GraphBrowserApp, tiles_tree: &Tree<TileKind>) {
    if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
        graph_app
            .record_workspace_undo_boundary(Some(layout_json), UndoBoundaryReason::OpenNodePane);
    }
}

fn anchor_before_tab_open(tiles_tree: &Tree<TileKind>, open_mode: TileOpenMode) -> Option<NodeKey> {
    if open_mode == TileOpenMode::Tab {
        nav_targeting::active_node_pane_node(tiles_tree)
    } else {
        None
    }
}

fn is_node_already_in_workspace(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> bool {
    tiles_tree.tiles.iter().any(|(_, tile)| {
        matches!(
            tile,
            egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == node_key
        )
    })
}

fn maybe_push_grouped_edge_after_tab_open(
    frame_intents: &mut Vec<GraphIntent>,
    open_mode: TileOpenMode,
    node_already_in_workspace: bool,
    anchor_before_open: Option<NodeKey>,
    node_key: NodeKey,
) {
    if open_mode == TileOpenMode::Tab
        && !node_already_in_workspace
        && let Some(anchor) = anchor_before_open
        && anchor != node_key
    {
        frame_intents.push(GraphIntent::CreateUserGroupedEdge {
            from: anchor,
            to: node_key,
            label: None,
        });
    }
}

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
/// `DetachNodeToSplit`, `SwapViewerBackend`, `SetPaneView`, `OpenNodeInPane`, tool-surface
/// toggles/settings URLs) must be drained here, before `apply_intents` is
/// called. Any that leak through will trip reducer hardening (panic in
/// debug/test, warning in release for non-layout authority leaks).
pub(crate) fn handle_tool_pane_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    workbench_intents: &mut Vec<WorkbenchIntent>,
) {
    handle_tool_pane_intents_with_modal_state(
        graph_app,
        tiles_tree,
        workbench_intents,
        modal_surface_active(graph_app),
    );
}

pub(crate) fn handle_tool_pane_intents_with_modal_state(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    workbench_intents: &mut Vec<WorkbenchIntent>,
    modal_surface_active: bool,
) {
    let mut remaining = Vec::with_capacity(workbench_intents.len());
    for intent in workbench_intents.drain(..) {
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
        if modal_surface_active && !modal_allows_workbench_intent(&intent) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_CONSUMED,
                byte_len: path.nodes.len(),
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED,
                byte_len: 1,
            });
            continue;
        }

        emit_dispatch_phase(UxDispatchPhase::Target);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_DISPATCH_PHASE,
            byte_len: UxDispatchPhase::Target as usize,
        });

        if let Some(unhandled) = dispatch_workbench_authority_intent(graph_app, tiles_tree, intent)
        {
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
    }
    *workbench_intents = remaining;
}

fn emit_dispatch_phase(phase: UxDispatchPhase) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_DISPATCH_PHASE,
        byte_len: phase as usize,
    });
}

fn modal_surface_active(graph_app: &GraphBrowserApp) -> bool {
    graph_app.workspace.show_command_palette
        || graph_app.workspace.show_radial_menu
        || graph_app.workspace.show_help_panel
}

fn modal_allows_workbench_intent(_intent: &WorkbenchIntent) -> bool {
    false
}

fn ux_event_kind_for_workbench_intent(intent: &WorkbenchIntent) -> UxEventKind {
    match intent {
        WorkbenchIntent::CycleFocusRegion => UxEventKind::FocusIn,
        _ => UxEventKind::Action,
    }
}

fn ux_dispatch_path_for_workbench_intent(intent: &WorkbenchIntent) -> UxDispatchPath {
    let leaf = match intent {
        WorkbenchIntent::OpenCommandPalette
        | WorkbenchIntent::ToggleCommandPalette
        | WorkbenchIntent::OpenToolPane { .. }
        | WorkbenchIntent::ClosePane { .. }
        | WorkbenchIntent::CloseToolPane { .. }
        | WorkbenchIntent::OpenSettingsUrl { .. }
        | WorkbenchIntent::OpenFrameUrl { .. }
        | WorkbenchIntent::OpenToolUrl { .. }
        | WorkbenchIntent::OpenViewUrl { .. }
        | WorkbenchIntent::OpenGraphUrl { .. }
        | WorkbenchIntent::OpenGraphViewPane { .. }
        | WorkbenchIntent::OpenNoteUrl { .. }
        | WorkbenchIntent::OpenNodeUrl { .. }
        | WorkbenchIntent::OpenClipUrl { .. }
        | WorkbenchIntent::SwapViewerBackend { .. }
        | WorkbenchIntent::SetPaneView { .. }
        | WorkbenchIntent::SplitPane { .. }
        | WorkbenchIntent::DetachNodeToSplit { .. }
        | WorkbenchIntent::OpenNodeInPane { .. }
        | WorkbenchIntent::CycleFocusRegion => UX_DISPATCH_NODE_TOOL_SURFACE,
    };

    UxDispatchPath {
        nodes: vec![UX_DISPATCH_NODE_ROOT, UX_DISPATCH_NODE_WORKBENCH, leaf],
    }
}

fn dispatch_workbench_authority_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    intent: WorkbenchIntent,
) -> Option<WorkbenchIntent> {
    crate::shell::desktop::runtime::registries::dispatch_workbench_surface_intent(
        graph_app, tiles_tree, intent,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_semantic_lifecycle_phase(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    modal_surface_active: bool,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    apply_semantic_intents_and_pending_open(
        graph_app,
        tiles_tree,
        modal_surface_active,
        open_node_tile_after_intents,
        frame_intents,
    );

    reconcile_semantic_lifecycle_phase(
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
        frame_intents,
    );
}

fn apply_semantic_intents_and_pending_open(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    modal_surface_active: bool,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let mut workbench_intents = graph_app.take_pending_workbench_intents();
    handle_tool_pane_intents_with_modal_state(
        graph_app,
        tiles_tree,
        &mut workbench_intents,
        modal_surface_active,
    );
    assert_workbench_intents_drained_before_reducer_apply(&workbench_intents);
    gui_frame::apply_intents_if_any(graph_app, tiles_tree, frame_intents);
    handle_pending_open_node_after_intents(
        graph_app,
        tiles_tree,
        open_node_tile_after_intents,
        frame_intents,
    );
    handle_pending_open_note_after_intents(graph_app, tiles_tree);
    handle_pending_open_clip_after_intents(graph_app, tiles_tree);
}

fn assert_workbench_intents_drained_before_reducer_apply(intents: &[WorkbenchIntent]) {
    if intents.is_empty() {
        return;
    }

    #[cfg(any(test, debug_assertions))]
    panic!(
        "workbench intents leaked past workbench-authority interception before reducer apply: {:?}",
        intents
    );

    #[cfg(not(any(test, debug_assertions)))]
    {
        log::warn!(
            "workbench intents leaked past workbench-authority interception before reducer apply; dropping {} leaked intent(s)",
            intents.len()
        );
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: intents.len(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn reconcile_semantic_lifecycle_phase(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    frame_intents: &mut Vec<GraphIntent>,
) {
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
            responsive_webviews,
            webview_creation_backpressure,
        },
        frame_intents,
    );
}

#[cfg(test)]
#[path = "gui_orchestration_tests.rs"]
mod gui_orchestration_tests;
