/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use arboard::Clipboard;

use crate::app::{
    ClipboardCopyKind, ClipboardCopyRequest, GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry,
    GraphSearchOrigin, GraphSearchRequest, LifecycleCause, PendingTileOpenMode, SearchDisplayMode,
    UndoBoundaryReason, WorkbenchIntent,
};
use crate::graph::NodeKey;
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
    CHANNEL_UX_FOCUS_REALIZATION_MISMATCH, CHANNEL_UX_FOCUS_RETURN_FALLBACK,
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_NAVIGATION_VIOLATION,
};
use crate::shell::desktop::ui::graph_search_flow::{self, GraphSearchFlowArgs};
use crate::shell::desktop::ui::graph_search_ui::{self, GraphSearchUiArgs};
use crate::shell::desktop::ui::gui_frame::ToolbarDialogPhaseArgs;
use crate::shell::desktop::ui::gui_frame::{self, PreFrameIngestArgs};
use crate::shell::desktop::ui::gui_state::{
    LocalFocusTarget, RuntimeFocusAuthorityState, ToolbarState,
};
use crate::shell::desktop::ui::nav_targeting;
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

#[path = "gui/focus_realizer.rs"]
mod focus_realizer;
#[path = "gui/graph_search_orchestration.rs"]
mod graph_search_orchestration;
#[path = "gui/workbench_intent_interceptor.rs"]
mod workbench_intent_interceptor;

use focus_realizer::FocusRealizer;

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
    toasts: &mut egui_notify::Toasts,
    graph_search_open: &mut bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    toolbar_state: &mut ToolbarState,
    frame_intents: &mut Vec<GraphIntent>,
    has_active_node_pane: bool,
) -> graph_search_flow::GraphSearchFlowOutput {
    graph_search_orchestration::run_graph_search_phase(
        ctx,
        graph_app,
        toasts,
        graph_search_open,
        local_widget_focus,
        graph_search_query,
        graph_search_filter_mode,
        graph_search_matches,
        graph_search_active_match_index,
        toolbar_state,
        frame_intents,
        has_active_node_pane,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_graph_search_window_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    graph_search_output: &mut graph_search_flow::GraphSearchFlowOutput,
) {
    graph_search_orchestration::run_graph_search_window_phase(
        ctx,
        graph_app,
        toolbar_visible,
        graph_search_open,
        is_graph_view,
        local_widget_focus,
        graph_search_query,
        graph_search_filter_mode,
        graph_search_matches,
        graph_search_active_match_index,
        graph_search_output,
    );
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

    let mut ranked =
        crate::shell::desktop::runtime::registries::phase3_index_search(graph_app, query, 64)
            .into_iter()
            .filter_map(|result| {
                match result.kind {
        crate::shell::desktop::runtime::registries::index::SearchResultKind::Node(key) => Some(key),
        crate::shell::desktop::runtime::registries::index::SearchResultKind::HistoryUrl(_)
        | crate::shell::desktop::runtime::registries::index::SearchResultKind::KnowledgeTag {
            ..
        } => None,
    }
            })
            .collect::<Vec<_>>();
    ranked.dedup();
    extend_matches_with_active_anchor_neighborhood(graph_app, &mut ranked);
    *matches = ranked;
    sync_graph_search_active_index(matches, active_index);
}

fn extend_matches_with_active_anchor_neighborhood(
    graph_app: &GraphBrowserApp,
    matches: &mut Vec<NodeKey>,
) {
    let Some(anchor) = graph_app.workspace.active_graph_search_neighborhood_anchor else {
        return;
    };
    if graph_app.domain_graph().get_node(anchor).is_none() {
        return;
    }

    let mut seen: HashSet<NodeKey> = matches.iter().copied().collect();
    let neighborhood_depth = graph_app
        .workspace
        .active_graph_search_neighborhood_depth
        .clamp(1, 2);
    for neighbor in std::iter::once(anchor).chain(
        graph_app
            .domain_graph()
            .connected_candidates_with_depth(anchor, neighborhood_depth)
            .into_iter()
            .map(|(neighbor, _)| neighbor),
    ) {
        if seen.insert(neighbor) {
            matches.push(neighbor);
        }
    }
}

fn maybe_push_graph_search_history(graph_app: &mut GraphBrowserApp, request: &GraphSearchRequest) {
    let previous_query = graph_app
        .workspace
        .active_graph_search_query
        .trim()
        .to_string();
    let previous_filter_mode = matches!(
        graph_app.workspace.search_display_mode,
        SearchDisplayMode::Filter
    );
    let previous_origin = graph_app.workspace.active_graph_search_origin.clone();
    let previous_neighborhood_anchor = graph_app.workspace.active_graph_search_neighborhood_anchor;
    let previous_neighborhood_depth = graph_app.workspace.active_graph_search_neighborhood_depth;

    if previous_query.is_empty() {
        return;
    }

    if previous_query == request.query
        && previous_filter_mode == request.filter_mode
        && previous_origin == request.origin
        && previous_neighborhood_anchor == request.neighborhood_anchor
        && previous_neighborhood_depth == request.neighborhood_depth
    {
        return;
    }

    let entry = GraphSearchHistoryEntry {
        query: previous_query,
        filter_mode: previous_filter_mode,
        origin: previous_origin,
        neighborhood_anchor: previous_neighborhood_anchor,
        neighborhood_depth: previous_neighborhood_depth,
    };

    graph_app
        .workspace
        .graph_search_history
        .retain(|existing| existing != &entry);
    graph_app.workspace.graph_search_history.push(entry);
    const GRAPH_SEARCH_HISTORY_LIMIT: usize = 5;
    if graph_app.workspace.graph_search_history.len() > GRAPH_SEARCH_HISTORY_LIMIT {
        let overflow = graph_app.workspace.graph_search_history.len() - GRAPH_SEARCH_HISTORY_LIMIT;
        graph_app.workspace.graph_search_history.drain(0..overflow);
    }
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

pub(crate) fn graph_search_toast_message(
    origin: GraphSearchOrigin,
    query: &str,
    neighborhood_depth: u8,
) -> String {
    let action = match origin {
        GraphSearchOrigin::Manual => "Search applied",
        GraphSearchOrigin::SemanticTag => "Semantic slice applied",
        GraphSearchOrigin::AnchorSlice => "Anchor slice applied",
    };
    let scope = if neighborhood_depth > 1 {
        format!(" ({}-hop neighborhood)", neighborhood_depth)
    } else if neighborhood_depth == 1 {
        " (1-hop neighborhood)".to_string()
    } else {
        String::new()
    };
    format!("{action}: {query}{scope}")
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
    local_widget_focus: &mut Option<LocalFocusTarget>,
    focus_authority: &RuntimeFocusAuthorityState,
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
            local_widget_focus,
            focus_authority,
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
    workbench_intent_interceptor::handle_tool_pane_intents(
        graph_app,
        tiles_tree,
        workbench_intents,
    );
}

pub(crate) fn handle_tool_pane_intents_with_modal_state(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    workbench_intents: &mut Vec<WorkbenchIntent>,
    modal_surface_active: bool,
) {
    workbench_intent_interceptor::handle_tool_pane_intents_with_modal_state(
        graph_app,
        tiles_tree,
        workbench_intents,
        modal_surface_active,
    );
}

fn handle_tool_pane_intents_with_modal_state_and_focus_authority(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    workbench_intents: &mut Vec<WorkbenchIntent>,
    modal_surface_active: bool,
    focus_authority: Option<&mut RuntimeFocusAuthorityState>,
) {
    workbench_intent_interceptor::handle_tool_pane_intents_with_modal_state_and_focus_authority(
        graph_app,
        tiles_tree,
        workbench_intents,
        modal_surface_active,
        focus_authority,
    );
}

fn refresh_runtime_focus_authority_after_workbench_intent(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    modal_surface_active: bool,
) {
    crate::shell::desktop::ui::gui::refresh_realized_runtime_focus_state(
        focus_authority,
        graph_app,
        tiles_tree,
        None,
        false,
    );
    let _ = modal_surface_active;
}

/// After the focus authority reducer and realizer have run for an authority-handled
/// intent, reconcile by syncing return targets and comparing desired vs observed
/// semantic region. Unlike `refresh_*`, this does NOT overwrite the authority's
/// `semantic_region` — the authority remains the source of truth. Mismatches
/// produce a `ux:focus_realization_mismatch` diagnostic.
fn reconcile_focus_authority_after_realization(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    modal_surface_active: bool,
) {
    crate::shell::desktop::ui::gui::refresh_realized_runtime_focus_state(
        focus_authority,
        graph_app,
        tiles_tree,
        None,
        false,
    );
    let _ = modal_surface_active;

    let desired_focus = crate::shell::desktop::ui::gui::desired_runtime_focus_state(
        graph_app,
        focus_authority,
        None,
        false,
    );
    if focus_authority
        .realized_focus_state
        .as_ref()
        .is_some_and(|realized| desired_focus.semantic_region != realized.semantic_region)
    {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_FOCUS_REALIZATION_MISMATCH,
            byte_len: 1,
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlannedSemanticWorkbenchRegion {
    GraphSurface,
    NodePane,
    #[cfg(feature = "diagnostics")]
    ToolPane,
}

fn planned_semantic_workbench_region_from_focus_authority(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    focus_authority: &RuntimeFocusAuthorityState,
) -> Option<crate::shell::desktop::ui::gui_state::SemanticRegionFocus> {
    let focus_state = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app,
        tiles_tree,
        Some(focus_authority),
        None,
        false,
    );
    let current = match focus_state.semantic_region {
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::GraphSurface { .. } => {
            Some(PlannedSemanticWorkbenchRegion::GraphSurface)
        }
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane { .. } => {
            Some(PlannedSemanticWorkbenchRegion::NodePane)
        }
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane { .. } => {
            #[cfg(feature = "diagnostics")]
            {
                Some(PlannedSemanticWorkbenchRegion::ToolPane)
            }
            #[cfg(not(feature = "diagnostics"))]
            {
                None
            }
        }
        _ => None,
    };
    let order = [
        PlannedSemanticWorkbenchRegion::GraphSurface,
        PlannedSemanticWorkbenchRegion::NodePane,
        #[cfg(feature = "diagnostics")]
        PlannedSemanticWorkbenchRegion::ToolPane,
    ];
    let start_index = current
        .and_then(|region| order.iter().position(|candidate| *candidate == region))
        .unwrap_or(order.len() - 1);

    for offset in 1..=order.len() {
        let candidate = order[(start_index + offset) % order.len()];
        let resolved = match candidate {
            PlannedSemanticWorkbenchRegion::GraphSurface => {
                tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                    egui_tiles::Tile::Pane(TileKind::Graph(view_ref)) => Some(
                        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::GraphSurface {
                            view_id: Some(view_ref.graph_view_id),
                        },
                    ),
                    _ => None,
                })
            }
            PlannedSemanticWorkbenchRegion::NodePane => {
                tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                    egui_tiles::Tile::Pane(TileKind::Node(state)) => Some(
                        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane {
                            pane_id: Some(state.pane_id),
                            node_key: Some(state.node),
                        },
                    ),
                    _ => None,
                })
            }
            #[cfg(feature = "diagnostics")]
            PlannedSemanticWorkbenchRegion::ToolPane => {
                tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                    egui_tiles::Tile::Pane(TileKind::Tool(state)) => Some(
                        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane {
                            pane_id: Some(state.pane_id),
                        },
                    ),
                    _ => None,
                })
            }
        };
        if resolved.is_some() {
            return resolved;
        }
    }

    None
}

fn prime_runtime_focus_authority_for_workbench_intent(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    intent: &WorkbenchIntent,
) {
    match intent {
        WorkbenchIntent::OpenCommandPalette => {
            let contextual_mode = matches!(
                focus_authority.semantic_region,
                Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette)
            );
            let return_target = if focus_authority.command_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.command_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterCommandPalette {
                    contextual_mode,
                    return_target,
                },
            );
            crate::shell::desktop::ui::gui::capture_command_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::ToggleCommandPalette if graph_app.workspace.show_command_palette => {
            crate::shell::desktop::ui::gui::seed_command_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitCommandPalette,
            );
        }
        WorkbenchIntent::ToggleCommandPalette => {
            let contextual_mode = matches!(
                focus_authority.semantic_region,
                Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette)
            );
            let return_target = if focus_authority.command_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.command_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterCommandPalette {
                    contextual_mode,
                    return_target,
                },
            );
            crate::shell::desktop::ui::gui::capture_command_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::ToggleHelpPanel if graph_app.workspace.show_help_panel => {
            crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitTransientSurface {
                    surface: crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                    restore_target: focus_authority.transient_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::ToggleHelpPanel => {
            let return_target = if focus_authority.transient_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.transient_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterTransientSurface {
                    surface: crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                    return_target,
                },
            );
        }
        WorkbenchIntent::ToggleRadialMenu if graph_app.workspace.show_radial_menu => {
            crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitTransientSurface {
                    surface:
                        crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                    restore_target: focus_authority.transient_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::ToggleRadialMenu => {
            let return_target = if focus_authority.transient_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.transient_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterTransientSurface {
                    surface:
                        crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                    return_target,
                },
            );
        }
        WorkbenchIntent::CycleFocusRegion => {
            if let Some(region) = planned_semantic_workbench_region_from_focus_authority(
                graph_app,
                tiles_tree,
                focus_authority,
            ) {
                crate::shell::desktop::ui::gui::apply_focus_command(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCommand::SetSemanticRegion {
                        region,
                    },
                );
            }
        }
        WorkbenchIntent::OpenToolPane { kind }
            if matches!(
                kind,
                ToolPaneState::Settings | ToolPaneState::HistoryManager
            ) =>
        {
            let return_target = if focus_authority.tool_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.tool_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterToolPane { return_target },
            );
            crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::OpenSettingsUrl { url } => {
            if settings_url_targets_overlay(tiles_tree, url) {
                let return_target = if focus_authority.transient_surface_return_target.is_none() {
                    crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
                } else {
                    focus_authority.transient_surface_return_target.clone()
                };
                crate::shell::desktop::ui::gui::apply_focus_command(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCommand::EnterTransientSurface {
                        surface:
                            crate::shell::desktop::ui::gui_state::FocusCaptureSurface::SettingsOverlay,
                        return_target,
                    },
                );
            } else {
                crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                    focus_authority,
                    tiles_tree,
                );
            }
        }
        WorkbenchIntent::OpenClipUrl { .. } => {
            crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::OpenToolUrl { url } => {
            if matches!(
                GraphBrowserApp::resolve_tool_route(url),
                Some(ToolPaneState::Settings | ToolPaneState::HistoryManager)
            ) {
                crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                    focus_authority,
                    tiles_tree,
                );
            }
        }
        WorkbenchIntent::ClosePane {
            restore_previous_focus: true,
            ..
        }
        | WorkbenchIntent::CloseToolPane {
            restore_previous_focus: true,
            ..
        } => {
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitToolPane {
                    restore_target: focus_authority.tool_surface_return_target.clone(),
                },
            );
            crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
        }
        _ => {}
    }
}

fn emit_dispatch_phase(phase: UxDispatchPhase) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_DISPATCH_PHASE,
        byte_len: phase as usize,
    });
}

fn settings_url_targets_overlay(tiles_tree: &Tree<TileKind>, url: &str) -> bool {
    matches!(
        GraphBrowserApp::resolve_settings_route(url),
        Some(crate::app::SettingsRouteTarget::Settings(_))
    ) && !tiles_tree.tiles.iter().any(|(_, tile)| {
        matches!(
            tile,
            egui_tiles::Tile::Pane(TileKind::Tool(tool)) if tool.kind == ToolPaneState::Settings
        )
    })
}

fn modal_surface_active(graph_app: &GraphBrowserApp) -> bool {
    modal_surface_active_with_focus_authority(graph_app, None)
}

fn modal_allows_workbench_intent(graph_app: &GraphBrowserApp, intent: &WorkbenchIntent) -> bool {
    modal_allows_workbench_intent_with_focus_authority(graph_app, intent, None)
}

fn modal_surface_active_with_focus_authority(
    graph_app: &GraphBrowserApp,
    focus_authority: Option<&RuntimeFocusAuthorityState>,
) -> bool {
    crate::shell::desktop::ui::gui::workspace_runtime_focus_state(
        graph_app,
        focus_authority,
        None,
        false,
    )
    .overlay_active()
}

fn modal_allows_workbench_intent_with_focus_authority(
    graph_app: &GraphBrowserApp,
    intent: &WorkbenchIntent,
    focus_authority: Option<&RuntimeFocusAuthorityState>,
) -> bool {
    let focus_state = crate::shell::desktop::ui::gui::workspace_runtime_focus_state(
        graph_app,
        focus_authority,
        None,
        false,
    );
    matches!(
        (intent, &focus_state.semantic_region),
        (
            WorkbenchIntent::ToggleCommandPalette,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::CommandPalette
                | crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette
        ) | (
            WorkbenchIntent::ToggleHelpPanel,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::HelpPanel
        ) | (
            WorkbenchIntent::ToggleRadialMenu,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::RadialPalette
        )
    )
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
        | WorkbenchIntent::ToggleHelpPanel
        | WorkbenchIntent::ToggleRadialMenu => UX_DISPATCH_NODE_COMMAND_SURFACE,
        WorkbenchIntent::OpenToolPane { .. }
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
        | WorkbenchIntent::SelectTile { .. }
        | WorkbenchIntent::UpdateTileSelection { .. }
        | WorkbenchIntent::ClearTileSelection
        | WorkbenchIntent::GroupSelectedTiles
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
    focus_authority: &mut RuntimeFocusAuthorityState,
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
        focus_authority,
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
    focus_authority: &mut RuntimeFocusAuthorityState,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    workbench_intent_interceptor::apply_semantic_intents_and_pending_open(
        graph_app,
        tiles_tree,
        modal_surface_active,
        focus_authority,
        open_node_tile_after_intents,
        frame_intents,
    );
}

fn restore_pending_transient_surface_focus(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_authority: &mut RuntimeFocusAuthorityState,
) {
    workbench_intent_interceptor::restore_pending_transient_surface_focus(
        graph_app,
        tiles_tree,
        focus_authority,
    );
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
