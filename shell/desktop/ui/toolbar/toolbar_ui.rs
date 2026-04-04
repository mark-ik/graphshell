/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crossbeam_channel::Receiver;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Key, Modifiers, TopBottomPanel, Vec2};
use egui_tiles::Tree;
use euclid::default::Point2D;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};
use winit::window::Window;

use crate::shell::desktop::runtime::control_panel::ControlPanel;
use crate::shell::desktop::runtime::protocols::router::{self, OutboundFetchError};
use crate::shell::desktop::ui::gui_state::FocusedContentStatus;
use crate::shell::desktop::ui::gui_state::LocalFocusTarget;
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::ui::workbench_host::WorkbenchLayerState;
use crate::shell::desktop::workbench::pane_model::{PaneId, ViewerId};
#[path = "toolbar_controls.rs"]
mod toolbar_controls;
#[path = "toolbar_location_dropdown.rs"]
mod toolbar_location_dropdown;
#[path = "toolbar_location_panel.rs"]
mod toolbar_location_panel;
#[path = "toolbar_location_submit.rs"]
mod toolbar_location_submit;
#[path = "toolbar_omnibar.rs"]
mod toolbar_omnibar;
#[path = "toolbar_right_controls.rs"]
mod toolbar_right_controls;
#[path = "toolbar_settings_menu.rs"]
mod toolbar_settings_menu;
use self::toolbar_controls::render_graph_history_buttons;
use self::toolbar_location_panel::render_location_search_panel;
use self::toolbar_omnibar::{
    apply_omnibar_match, dedupe_matches_in_order, default_search_provider_from_searchpage,
    graph_center_for_new_node, non_at_global_fallback_matches, non_at_matches_for_settings,
    non_at_primary_matches_for_scope, omnibar_match_label, omnibar_match_signifier,
    omnibar_matches_for_query, parse_omnibar_search_query, parse_provider_search_query,
    searchpage_template_for_provider, spawn_provider_suggestion_request,
};
use self::toolbar_right_controls::render_toolbar_right_controls;
use self::toolbar_settings_menu::render_settings_menu;
use crate::app::{
    CommandPaletteShortcut, GraphBrowserApp, GraphIntent, GraphViewId, HelpPanelShortcut,
    OmnibarNonAtOrderPreset, OmnibarPreferredScope, PendingTileOpenMode, RadialMenuShortcut,
    ToastAnchorPreference, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::services::search::{fuzzy_match_items, fuzzy_match_node_keys};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::registries::lens::{LENS_ID_DEFAULT, LENS_ID_SEMANTIC_OVERLAY};
use crate::shell::desktop::runtime::registries::{
    input::action_id, phase2_binding_display_labels_for_action,
    CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
    CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
    CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
    CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED,
    CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE,
};
use crate::shell::desktop::ui::navigator_context::NavigatorContextProjection;
use crate::shell::desktop::workbench::tile_kind::TileKind;

const WORKSPACE_PIN_NAME: &str = "workspace:pin:space";
const OMNIBAR_DROPDOWN_MAX_ROWS: usize = 8;
const OMNIBAR_PROVIDER_MIN_QUERY_LEN: usize = 2;
const OMNIBAR_CONNECTED_NON_AT_CAP: usize = 8;
const OMNIBAR_GLOBAL_NODES_FALLBACK_CAP: usize = 3;
const OMNIBAR_GLOBAL_TABS_FALLBACK_CAP: usize = 3;
const OMNIBAR_PROVIDER_DEBOUNCE_MS: u64 = 140;
/// Fixed height of the top chrome bar. All columns within the bar must fit within
/// this budget; content that exceeds it is clipped rather than allowed to grow.
const TOOLBAR_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OmnibarSessionKind {
    Graph(OmnibarSearchMode),
    SearchProvider(SearchProviderKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SearchProviderKind {
    DuckDuckGo,
    Bing,
    Google,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OmnibarSearchMode {
    Mixed,
    NodesLocal,
    NodesAll,
    TabsLocal,
    TabsAll,
    EdgesLocal,
    EdgesAll,
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalNodeMatch {
    pub(crate) url: String,
    pub(crate) display_label: Option<String>,
}

impl HistoricalNodeMatch {
    pub(crate) fn new(url: impl Into<String>, display_label: Option<String>) -> Self {
        Self {
            url: url.into(),
            display_label,
        }
    }

    pub(crate) fn without_label(url: impl Into<String>) -> Self {
        Self::new(url, None)
    }
}

impl PartialEq for HistoricalNodeMatch {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Eq for HistoricalNodeMatch {}

impl Hash for HistoricalNodeMatch {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OmnibarMatch {
    Node(NodeKey),
    NodeUrl(HistoricalNodeMatch),
    SearchQuery {
        query: String,
        provider: SearchProviderKind,
    },
    Edge {
        from: NodeKey,
        to: NodeKey,
    },
    /// A durable graphlet peer of a warm node that is currently `Cold` (no live tile).
    /// Shown with ○ in the `TabsLocal` empty-query roster; activating opens a tile via
    /// graphlet routing.
    ColdGraphletMember(NodeKey),
}

#[derive(Clone)]
struct OmnibarSearchCandidate {
    text: String,
    target: OmnibarMatch,
}

impl AsRef<str> for OmnibarSearchCandidate {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

pub(crate) struct OmnibarSearchSession {
    kind: OmnibarSessionKind,
    pub(crate) query: String,
    pub(crate) matches: Vec<OmnibarMatch>,
    pub(crate) active_index: usize,
    selected_indices: HashSet<usize>,
    anchor_index: Option<usize>,
    provider_mailbox: ProviderSuggestionMailbox,
}

struct ProviderSuggestionMailbox {
    request_query: Option<String>,
    rx: Option<Receiver<ProviderSuggestionFetchOutcome>>,
    debounce_deadline: Option<Instant>,
    status: ProviderSuggestionStatus,
}

impl ProviderSuggestionMailbox {
    fn idle() -> Self {
        Self {
            request_query: None,
            rx: None,
            debounce_deadline: None,
            status: ProviderSuggestionStatus::Idle,
        }
    }

    fn debounced(request_query: String, debounce_deadline: Instant) -> Self {
        Self {
            request_query: Some(request_query),
            rx: None,
            debounce_deadline: Some(debounce_deadline),
            status: ProviderSuggestionStatus::Loading,
        }
    }

    fn ready() -> Self {
        Self {
            status: ProviderSuggestionStatus::Ready,
            ..Self::idle()
        }
    }

    fn clear_pending(&mut self) {
        self.request_query = None;
        self.rx = None;
        self.debounce_deadline = None;
    }
}

impl OmnibarSearchSession {
    fn new_graph(
        kind: OmnibarSearchMode,
        query: impl Into<String>,
        matches: Vec<OmnibarMatch>,
    ) -> Self {
        Self {
            kind: OmnibarSessionKind::Graph(kind),
            query: query.into(),
            matches,
            active_index: 0,
            selected_indices: HashSet::new(),
            anchor_index: None,
            provider_mailbox: ProviderSuggestionMailbox::idle(),
        }
    }

    fn new_search_provider(
        provider: SearchProviderKind,
        query: impl Into<String>,
        matches: Vec<OmnibarMatch>,
        provider_mailbox: ProviderSuggestionMailbox,
    ) -> Self {
        Self {
            kind: OmnibarSessionKind::SearchProvider(provider),
            query: query.into(),
            matches,
            active_index: 0,
            selected_indices: HashSet::new(),
            anchor_index: None,
            provider_mailbox,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderSuggestionStatus {
    Idle,
    Loading,
    Ready,
    Failed(ProviderSuggestionError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderSuggestionError {
    Network,
    HttpStatus(u16),
    Parse,
}

struct ProviderSuggestionFetchOutcome {
    matches: Vec<OmnibarMatch>,
    status: ProviderSuggestionStatus,
}

pub(crate) struct Input<'a> {
    pub ctx: &'a egui::Context,
    pub winit_window: &'a Window,
    pub state: &'a RunningAppState,
    pub graph_app: &'a mut GraphBrowserApp,
    pub control_panel: &'a mut ControlPanel,
    pub window: &'a EmbedderWindow,
    pub tiles_tree: &'a Tree<TileKind>,
    pub navigator_ctx: &'a NavigatorContextProjection,
    pub focused_toolbar_node: Option<NodeKey>,
    pub active_toolbar_pane: Option<PaneId>,
    pub workbench_layer_state: WorkbenchLayerState,
    pub focused_content_status: &'a FocusedContentStatus,
    pub local_widget_focus: &'a mut Option<LocalFocusTarget>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub location: &'a mut String,
    pub location_dirty: &'a mut bool,
    pub location_submitted: &'a mut bool,
    pub focus_location_field_for_search: bool,
    pub show_clear_data_confirm: &'a mut bool,
    pub omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub frame_intents: &'a mut Vec<GraphIntent>,
    #[cfg(feature = "diagnostics")]
    pub diagnostics_state: &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
}

pub(crate) struct Output {
    pub toggle_tile_view_requested: bool,
    pub open_selected_mode_after_submit: Option<ToolbarOpenMode>,
    pub toolbar_visible: bool,
    pub command_bar_rect: Option<egui::Rect>,
}

pub(crate) type ToolbarUiInput<'a> = Input<'a>;
pub(crate) type ToolbarUiOutput = Output;

fn toolbar_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text)
        .frame(false)
        .min_size(Vec2 { x: 20.0, y: 20.0 })
}

fn toast_anchor_label(anchor: ToastAnchorPreference) -> &'static str {
    match anchor {
        ToastAnchorPreference::TopRight => "Top Right",
        ToastAnchorPreference::TopLeft => "Top Left",
        ToastAnchorPreference::BottomRight => "Bottom Right (Default)",
        ToastAnchorPreference::BottomLeft => "Bottom Left",
    }
}

fn command_palette_shortcut_label(shortcut: CommandPaletteShortcut) -> &'static str {
    match shortcut {
        CommandPaletteShortcut::F2 => "F2 (Default)",
        CommandPaletteShortcut::CtrlK => "Ctrl+K",
    }
}

fn help_shortcut_label(shortcut: HelpPanelShortcut) -> &'static str {
    match shortcut {
        HelpPanelShortcut::F1OrQuestion => "F1 / ? (Default)",
        HelpPanelShortcut::H => "H",
    }
}

fn radial_shortcut_label(shortcut: RadialMenuShortcut) -> &'static str {
    match shortcut {
        RadialMenuShortcut::F3 => "F3 (Default)",
        RadialMenuShortcut::R => "R",
    }
}

fn lasso_binding_label(binding: CanvasLassoBinding) -> &'static str {
    match binding {
        CanvasLassoBinding::RightDrag => "Right Drag (Default)",
        CanvasLassoBinding::ShiftLeftDrag => "Shift + Left Drag",
    }
}

fn omnibar_preferred_scope_label(scope: OmnibarPreferredScope) -> &'static str {
    match scope {
        OmnibarPreferredScope::Auto => "Auto (Contextual)",
        OmnibarPreferredScope::LocalTabs => "Local Tabs First",
        OmnibarPreferredScope::ConnectedNodes => "Connected Nodes First",
        OmnibarPreferredScope::ProviderDefault => "Provider First",
        OmnibarPreferredScope::GlobalNodes => "Global Nodes First",
        OmnibarPreferredScope::GlobalTabs => "Global Tabs First",
    }
}

fn omnibar_non_at_order_label(order: OmnibarNonAtOrderPreset) -> &'static str {
    match order {
        OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal => {
            "Contextual -> Provider -> Global (Default)"
        }
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => {
            "Provider -> Contextual -> Global"
        }
    }
}

fn provider_status_label(status: ProviderSuggestionStatus) -> Option<String> {
    match status {
        ProviderSuggestionStatus::Idle => None,
        ProviderSuggestionStatus::Loading => Some("Suggestions: loading...".to_string()),
        ProviderSuggestionStatus::Ready => None,
        ProviderSuggestionStatus::Failed(ProviderSuggestionError::Network) => {
            Some("Suggestions unavailable: network error".to_string())
        }
        ProviderSuggestionStatus::Failed(ProviderSuggestionError::HttpStatus(code)) => {
            Some(format!("Suggestions unavailable: provider http {code}"))
        }
        ProviderSuggestionStatus::Failed(ProviderSuggestionError::Parse) => {
            Some("Suggestions unavailable: response parse error".to_string())
        }
    }
}

pub(super) fn emit_command_bar_command_palette_requested() {
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
            byte_len: "command_palette".len(),
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_request_started(query: &str) {
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED,
            byte_len: query.trim().len().max(1),
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_applied() {
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
            latency_us: 0,
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_failed() {
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
            byte_len: 1,
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_stale() {
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE,
            byte_len: 1,
        },
    );
}

fn request_open_settings_page(
    graph_app: &mut GraphBrowserApp,
    frame_intents: &mut Vec<GraphIntent>,
    url: &str,
) {
    frame_intents.push(GraphIntent::CreateNodeAtUrlAndOpen {
        url: url.to_string(),
        position: graph_center_for_new_node(graph_app),
        mode: PendingTileOpenMode::Tab,
    });
}

fn frame_pin_name_for_node(node: NodeKey, graph_app: &GraphBrowserApp) -> Option<String> {
    graph_app
        .domain_graph()
        .get_node(node)
        .map(|n| format!("workspace:pin:pane:{}", n.id))
}

fn render_fullscreen_origin_strip(
    ctx: &egui::Context,
    graph_app: &GraphBrowserApp,
    focused_toolbar_node: Option<NodeKey>,
) {
    let fullscreen_url = focused_toolbar_node
        .and_then(|key| {
            graph_app
                .domain_graph()
                .get_node(key)
                .map(|node| node.url().to_string())
        })
        .unwrap_or_else(|| "about:blank".to_string());
    let frame = egui::Frame::default()
        .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 25, 220))
        .inner_margin(4.0);
    TopBottomPanel::top("fullscreen_origin_strip")
        .frame(frame)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Fullscreen");
                ui.separator();
                ui.label(fullscreen_url);
                ui.separator();
                ui.label("Press Esc to exit");
            });
        });
}

/// Render the graph view tab strip from the Navigator context projection.
///
/// Navigator owns the content (view list, active view); Shell renders it here.
/// Single view: shows a compact "View: {name}" label (no tab strip needed).
/// Multiple views: renders selectable tab labels from `navigator_ctx.extra_views`.
fn render_navigator_view_tabs(
    ui: &mut egui::Ui,
    navigator_ctx: &NavigatorContextProjection,
    frame_intents: &mut Vec<GraphIntent>,
) {
    if navigator_ctx.extra_views.is_empty() {
        // Single view or no views: show a compact label.
        let label = navigator_ctx
            .active_view
            .as_ref()
            .map(|(_, name)| format!("View: {name}"))
            .unwrap_or_else(|| "View: Graph".to_string());
        ui.label(label);
    } else {
        // Multi-view: render the active tab first, then the rest.
        if let Some((_view_id, label)) = &navigator_ctx.active_view {
            let _ = ui.selectable_label(true, label.as_str());
        }
        for (view_id, label) in &navigator_ctx.extra_views {
            if ui.selectable_label(false, label.as_str()).clicked() {
                frame_intents.push(GraphIntent::FocusGraphView { view_id: *view_id });
            }
        }
    }
}

fn render_wry_compat_button(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    focused_toolbar_node: Option<NodeKey>,
    active_toolbar_pane: Option<PaneId>,
) {
    // Only show when a node pane is focused and wry is available.
    let (Some(node_key), Some(pane_id)) = (focused_toolbar_node, active_toolbar_pane) else {
        return;
    };
    if !cfg!(feature = "wry")
        || !crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry")
        || !graph_app.wry_enabled()
    {
        return;
    }

    // Find the active pane's current viewer_id_override.
    let current_viewer = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        egui_tiles::Tile::Pane(TileKind::Node(state)) if state.pane_id == pane_id => {
            Some(state.viewer_id_override.clone())
        }
        _ => None,
    });
    let Some(viewer_id_override) = current_viewer else {
        return;
    };

    let wry_active = viewer_id_override
        .as_ref()
        .is_some_and(|v| v.as_str() == "viewer:wry");

    let button = ui
        .add(toolbar_button(if wry_active { "Servo" } else { "Compat" }))
        .on_hover_text(if wry_active {
            "Switch back to Servo renderer"
        } else {
            "Load in Wry compatibility mode"
        });
    if button.clicked() {
        let new_override = if wry_active {
            Some(ViewerId::new("viewer:webview"))
        } else {
            Some(ViewerId::new("viewer:wry"))
        };
        graph_app.enqueue_workbench_intent(WorkbenchIntent::SwapViewerBackend {
            pane: pane_id,
            node: node_key,
            viewer_id_override: new_override,
        });
    }
}

fn graph_bar_lens_label(graph_app: &GraphBrowserApp) -> String {
    let view_id = active_graph_view_id(graph_app);
    let lens_id = view_id
        .and_then(|id| graph_app.workspace.graph_runtime.views.get(&id))
        .and_then(|view| view.resolved_lens_id().map(str::to_owned))
        .or_else(|| graph_app.default_registry_lens_id().map(str::to_owned))
        .unwrap_or_else(|| LENS_ID_DEFAULT.to_string());
    format!("Lens: {}", lens_id.trim_start_matches("lens:"))
}

fn graph_bar_physics_label(graph_app: &GraphBrowserApp) -> String {
    let physics_id = active_graph_view_id(graph_app)
        .and_then(|id| graph_app.workspace.graph_runtime.views.get(&id))
        .and_then(|view| view.resolved_physics_profile_id().map(str::to_owned))
        .or_else(|| graph_app.default_registry_physics_id().map(str::to_owned))
        .unwrap_or_else(|| crate::registries::atomic::lens::PHYSICS_ID_DEFAULT.to_string());
    let resolution = crate::registries::atomic::lens::resolve_physics_profile(&physics_id);
    format!("Physics: {}", resolution.display_name)
}

fn active_graph_view_id(graph_app: &GraphBrowserApp) -> Option<crate::app::GraphViewId> {
    graph_app.workspace.graph_runtime.focused_view.or_else(|| {
        (graph_app.workspace.graph_runtime.views.len() == 1)
            .then(|| {
                graph_app
                    .workspace
                    .graph_runtime
                    .views
                    .keys()
                    .next()
                    .copied()
            })
            .flatten()
    })
}

fn render_graph_bar_lens_menu(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    frame_intents: &mut Vec<GraphIntent>,
) {
    ui.menu_button(graph_bar_lens_label(graph_app), |ui| {
        let Some(view_id) = active_graph_view_id(graph_app) else {
            ui.label("No active graph view");
            return;
        };
        if !graph_app
            .workspace
            .graph_runtime
            .views
            .contains_key(&view_id)
        {
            ui.label("No active graph view");
            return;
        }

        for (label, lens_id) in [
            ("Default", LENS_ID_DEFAULT),
            ("Semantic Overlay", LENS_ID_SEMANTIC_OVERLAY),
        ] {
            if ui.button(label).clicked() {
                frame_intents.push(GraphIntent::SetViewLensId {
                    view_id,
                    lens_id: lens_id.to_string(),
                });
                ui.close();
            }
        }
    });
}

fn render_graph_bar_physics_menu(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    frame_intents: &mut Vec<GraphIntent>,
) {
    ui.menu_button(graph_bar_physics_label(graph_app), |ui| {
        let active_view_id = active_graph_view_id(graph_app).filter(|view_id| {
            graph_app
                .workspace
                .graph_runtime
                .views
                .contains_key(view_id)
        });
        for descriptor in crate::registries::atomic::lens::physics_profile_descriptors() {
            if ui.button(descriptor.display_name.as_str()).clicked() {
                if let Some(view_id) = active_view_id {
                    frame_intents.push(GraphIntent::SetViewPhysicsProfile {
                        view_id,
                        profile_id: descriptor.id.clone(),
                    });
                } else {
                    frame_intents.push(GraphIntent::SetPhysicsProfile {
                        profile_id: descriptor.id.clone(),
                    });
                }
                ui.close();
            }
        }
        ui.separator();
        let running = graph_app.workspace.graph_runtime.physics.base.is_running;
        let toggle_label = if running {
            "Pause Physics"
        } else {
            "Resume Physics"
        };
        if ui.button(toggle_label).clicked() {
            graph_app.workspace.graph_runtime.physics.base.is_running = !running;
            if !running {
                frame_intents.push(GraphIntent::ReheatPhysics);
            }
            ui.close();
        }
        if ui.button("Reheat Physics").clicked() {
            frame_intents.push(GraphIntent::ReheatPhysics);
            ui.close();
        }
    });
}

fn open_selected_node_tag_panel(graph_app: &mut GraphBrowserApp) {
    let Some(node_key) = graph_app.focused_selection().primary() else {
        return;
    };
    crate::shell::desktop::ui::tag_panel::open_node_tag_panel(graph_app, node_key, false);
}

fn overview_plane_tooltip(graph_app: &GraphBrowserApp) -> String {
    let shortcut =
        phase2_binding_display_labels_for_action(action_id::graph::TOGGLE_OVERVIEW_PLANE)
            .into_iter()
            .next()
            .unwrap_or_else(|| "Ctrl+Shift+O".to_string());
    if graph_app.graph_view_layout_manager_active() {
        format!("Exit Overview Plane ({shortcut})")
    } else {
        format!("Open Overview Plane ({shortcut})")
    }
}

fn render_command_bar_navigator_projection_host(
    ui: &mut egui::Ui,
    navigator_ctx: &NavigatorContextProjection,
    frame_intents: &mut Vec<GraphIntent>,
) {
    render_navigator_view_tabs(ui, navigator_ctx, frame_intents);
}

fn render_command_bar_legacy_graph_actions(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    focused_toolbar_node: Option<NodeKey>,
    active_toolbar_pane: Option<PaneId>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    render_wry_compat_button(
        ui,
        graph_app,
        tiles_tree,
        focused_toolbar_node,
        active_toolbar_pane,
    );
    render_graph_history_buttons(ui, frame_intents);

    let new_node_button = ui
        .add(toolbar_button("+Node"))
        .on_hover_text("Create node and open as tab");
    if new_node_button.clicked() {
        frame_intents.push(GraphIntent::CreateNodeNearCenterAndOpen {
            mode: PendingTileOpenMode::Tab,
        });
    }

    let new_edge_button = ui
        .add(toolbar_button("+Edge"))
        .on_hover_text("Create user-grouped edge from primary selection");
    if new_edge_button.clicked() {
        frame_intents.push(GraphIntent::CreateUserGroupedEdgeFromPrimarySelection);
    }

    let add_tag_button = ui
        .add_enabled(
            graph_app.focused_selection().primary().is_some(),
            toolbar_button("+Tag"),
        )
        .on_hover_text("Edit tags for the selected node");
    if add_tag_button.clicked() {
        open_selected_node_tag_panel(graph_app);
    }

    render_graph_bar_lens_menu(ui, graph_app, frame_intents);
    render_graph_bar_physics_menu(ui, graph_app, frame_intents);

    let overview_label = if graph_app.graph_view_layout_manager_active() {
        "Overview*"
    } else {
        "Overview"
    };
    let overview_button = ui
        .add(toolbar_button(overview_label))
        .on_hover_text(overview_plane_tooltip(graph_app));
    if overview_button.clicked() {
        frame_intents.push(GraphIntent::ToggleGraphViewLayoutManager);
    }
}

fn render_command_bar_shell_actions(ui: &mut egui::Ui, graph_app: &mut GraphBrowserApp) {
    let command_button = ui
        .add(toolbar_button("Cmd"))
        .on_hover_text("Open command palette (F2)");
    if command_button.clicked() {
        emit_command_bar_command_palette_requested();
        graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleCommandPalette);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_command_bar_left_column(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    navigator_ctx: &NavigatorContextProjection,
    focused_toolbar_node: Option<NodeKey>,
    active_toolbar_pane: Option<PaneId>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    render_command_bar_navigator_projection_host(ui, navigator_ctx, frame_intents);
    render_command_bar_legacy_graph_actions(
        ui,
        graph_app,
        tiles_tree,
        focused_toolbar_node,
        active_toolbar_pane,
        frame_intents,
    );
    render_command_bar_shell_actions(ui, graph_app);
}

#[allow(clippy::too_many_arguments)]
fn render_command_bar_right_column(
    ui: &mut egui::Ui,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    focused_toolbar_node: Option<NodeKey>,
    focused_content_status: &FocusedContentStatus,
    is_graph_view: bool,
    location_dirty: &mut bool,
    show_clear_data_confirm: &mut bool,
    frame_intents: &mut Vec<GraphIntent>,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) {
    ui.horizontal(|ui| {
        toolbar_controls::render_navigation_buttons(
            ui,
            graph_app,
            window,
            focused_toolbar_node,
            focused_content_status,
            location_dirty,
        );
    });
    render_toolbar_right_controls(
        ui,
        state,
        graph_app,
        window,
        is_graph_view,
        location_dirty,
        show_clear_data_confirm,
        frame_intents,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    );
}

pub(crate) fn render_toolbar_ui(args: Input<'_>) -> Output {
    let Input {
        ctx,
        winit_window,
        state,
        graph_app,
        control_panel,
        window,
        tiles_tree,
        navigator_ctx,
        focused_toolbar_node,
        active_toolbar_pane,
        workbench_layer_state,
        focused_content_status,
        local_widget_focus,
        can_go_back: _,
        can_go_forward: _,
        location,
        location_dirty,
        location_submitted,
        focus_location_field_for_search,
        show_clear_data_confirm,
        omnibar_search_session,
        frame_intents,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    } = args;

    if winit_window.fullscreen().is_some() {
        render_fullscreen_origin_strip(ctx, graph_app, focused_toolbar_node);
        return Output {
            toggle_tile_view_requested: false,
            open_selected_mode_after_submit: None,
            toolbar_visible: false,
            command_bar_rect: None,
        };
    }

    let toggle_tile_view_requested = false;
    let mut open_selected_mode_after_submit = None;
    let is_graph_view = matches!(
        workbench_layer_state,
        WorkbenchLayerState::GraphOnly | WorkbenchLayerState::GraphOverlayActive
    );
    let has_node_panes = !is_graph_view;
    let frame = egui::Frame::default()
        .fill(ctx.style().visuals.window_fill)
        .inner_margin(4.0);
    let command_bar_response = TopBottomPanel::top("shell_command_bar")
        .frame(frame)
        .exact_height(TOOLBAR_HEIGHT)
        .show(ctx, |ui| {
            ui.columns(3, |columns| {
                columns[0].horizontal_wrapped(|ui| {
                    render_command_bar_left_column(
                        ui,
                        graph_app,
                        tiles_tree,
                        navigator_ctx,
                        focused_toolbar_node,
                        active_toolbar_pane,
                        frame_intents,
                    );
                });

                columns[1].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    render_location_search_panel(
                        ui,
                        ctx,
                        state,
                        graph_app,
                        control_panel,
                        window,
                        tiles_tree,
                        focused_toolbar_node,
                        active_toolbar_pane,
                        local_widget_focus,
                        has_node_panes,
                        is_graph_view,
                        navigator_ctx,
                        location,
                        location_dirty,
                        location_submitted,
                        focus_location_field_for_search,
                        omnibar_search_session,
                        frame_intents,
                        &mut open_selected_mode_after_submit,
                    );
                });

                columns[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_command_bar_right_column(
                        ui,
                        state,
                        graph_app,
                        window,
                        focused_toolbar_node,
                        focused_content_status,
                        is_graph_view,
                        location_dirty,
                        show_clear_data_confirm,
                        frame_intents,
                        #[cfg(feature = "diagnostics")]
                        diagnostics_state,
                    );
                });
            });
        });

    Output {
        toggle_tile_view_requested,
        open_selected_mode_after_submit,
        toolbar_visible: true,
        command_bar_rect: Some(command_bar_response.response.rect),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        emit_command_bar_command_palette_requested, emit_omnibar_provider_mailbox_applied,
        emit_omnibar_provider_mailbox_failed, emit_omnibar_provider_mailbox_request_started,
        emit_omnibar_provider_mailbox_stale,
    };
    use crate::shell::desktop::runtime::diagnostics::{
        DiagnosticEvent, install_global_sender,
    };
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE,
    };

    #[test]
    fn command_bar_command_palette_request_emits_diagnostic() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);

        emit_command_bar_command_palette_requested();

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED
            )),
            "expected command bar dispatch diagnostic; got: {emitted:?}"
        );
    }

    #[test]
    fn omnibar_provider_mailbox_helpers_emit_diagnostics() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);

        emit_omnibar_provider_mailbox_request_started("rust async");
        emit_omnibar_provider_mailbox_applied();
        emit_omnibar_provider_mailbox_failed();
        emit_omnibar_provider_mailbox_stale();

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED
            )),
            "expected provider mailbox request-started diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageReceived { channel_id, .. }
                    if *channel_id == CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED
            )),
            "expected provider mailbox applied diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED
            )),
            "expected provider mailbox failed diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE
            )),
            "expected provider mailbox stale diagnostic; got: {emitted:?}"
        );
    }
}
