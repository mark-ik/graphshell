/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Key, Modifiers, TopBottomPanel, Vec2};
use egui_tiles::Tree;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use winit::window::Window;

use crate::shell::desktop::runtime::control_panel::{
    ControlPanel, HostRequestMailbox, HostRequestPoll,
};
use crate::shell::desktop::runtime::protocols::router::{self, OutboundFetchError};
use crate::shell::desktop::ui::gui_state::{
    FocusedContentStatus, LocalFocusTarget, RuntimeFocusState,
};
use crate::shell::desktop::ui::toolbar_routing::{self, ToolbarOpenMode};
use crate::shell::desktop::ui::workbench_host::WorkbenchLayerState;
use crate::shell::desktop::workbench::pane_model::PaneId;
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
#[path = "toolbar_status_bar.rs"]
mod toolbar_status_bar;
#[path = "toolbar_settings_menu.rs"]
mod toolbar_settings_menu;
use self::toolbar_location_panel::render_location_search_panel;
use self::toolbar_omnibar::{
    apply_omnibar_match, dedupe_matches_in_order, default_search_provider_from_searchpage,
    graph_center_for_new_node, non_at_global_fallback_matches, non_at_matches_for_settings,
    non_at_primary_matches_for_scope, omnibar_match_label, omnibar_match_signifier,
    omnibar_matches_for_query, parse_omnibar_search_query, parse_provider_search_query,
    searchpage_template_for_provider, spawn_provider_suggestion_request,
};
use self::toolbar_right_controls::render_toolbar_right_controls;
use self::toolbar_status_bar::render_shell_status_bar;
use self::toolbar_settings_menu::render_settings_menu;
use crate::app::{
    CommandPaletteShortcut, GraphBrowserApp, GraphIntent, GraphViewId, HelpPanelShortcut,
    OmnibarNonAtOrderPreset, OmnibarPreferredScope, PendingTileOpenMode, RadialMenuShortcut,
    ToastAnchorPreference, ToolSurfaceReturnTarget, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::services::search::{fuzzy_match_items, fuzzy_match_node_keys};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::registries::{
    input::action_id, phase2_binding_display_labels_for_action,
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
    result_mailbox: HostRequestMailbox<ProviderSuggestionFetchOutcome>,
    debounce_deadline: Option<Instant>,
    status: ProviderSuggestionStatus,
}

impl ProviderSuggestionMailbox {
    fn idle() -> Self {
        Self {
            request_query: None,
            result_mailbox: HostRequestMailbox::idle(),
            debounce_deadline: None,
            status: ProviderSuggestionStatus::Idle,
        }
    }

    fn debounced(request_query: String, debounce_deadline: Instant) -> Self {
        Self {
            request_query: Some(request_query),
            result_mailbox: HostRequestMailbox::idle(),
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
        self.result_mailbox.clear();
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandBarSemanticMetadata {
    pub(crate) active_pane: Option<PaneId>,
    pub(crate) focused_node: Option<NodeKey>,
    pub(crate) location_focused: bool,
    pub(crate) route_events: CommandRouteEventSequenceMetadata,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandRouteEventSequenceMetadata {
    pub(crate) resolved: u64,
    pub(crate) fallback: u64,
    pub(crate) no_target: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OmnibarMailboxEventSequenceMetadata {
    pub(crate) request_started: u64,
    pub(crate) applied: u64,
    pub(crate) failed: u64,
    pub(crate) stale: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandSurfaceEventSequenceMetadata {
    pub(crate) route_events: CommandRouteEventSequenceMetadata,
    pub(crate) omnibar_mailbox_events: OmnibarMailboxEventSequenceMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OmnibarSemanticMetadata {
    pub(crate) active: bool,
    pub(crate) focused: bool,
    pub(crate) query: Option<String>,
    pub(crate) match_count: usize,
    pub(crate) provider_status: Option<String>,
    pub(crate) active_pane: Option<PaneId>,
    pub(crate) focused_node: Option<NodeKey>,
    pub(crate) mailbox_events: OmnibarMailboxEventSequenceMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PaletteSurfaceSemanticMetadata {
    pub(crate) contextual_mode: bool,
    pub(crate) return_target: Option<ToolSurfaceReturnTarget>,
    pub(crate) pending_node_context_target: Option<NodeKey>,
    pub(crate) pending_frame_context_target: Option<String>,
    pub(crate) context_anchor_present: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandSurfaceSemanticSnapshot {
    pub(crate) command_bar: CommandBarSemanticMetadata,
    pub(crate) omnibar: OmnibarSemanticMetadata,
    pub(crate) command_palette: Option<PaletteSurfaceSemanticMetadata>,
    pub(crate) context_palette: Option<PaletteSurfaceSemanticMetadata>,
}

static LATEST_COMMAND_SURFACE_SEMANTIC_SNAPSHOT: OnceLock<Mutex<Option<CommandSurfaceSemanticSnapshot>>> =
    OnceLock::new();
static COMMAND_SURFACE_EVENT_SEQUENCES: OnceLock<Mutex<CommandSurfaceEventSequenceMetadata>> =
    OnceLock::new();

fn command_surface_snapshot_cache() -> &'static Mutex<Option<CommandSurfaceSemanticSnapshot>> {
    LATEST_COMMAND_SURFACE_SEMANTIC_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

fn command_surface_event_sequence_cache(
) -> &'static Mutex<CommandSurfaceEventSequenceMetadata> {
    COMMAND_SURFACE_EVENT_SEQUENCES
        .get_or_init(|| Mutex::new(CommandSurfaceEventSequenceMetadata::default()))
}

fn update_command_surface_event_sequences(
    mutator: impl FnOnce(&mut CommandSurfaceEventSequenceMetadata),
) {
    if let Ok(mut state) = command_surface_event_sequence_cache().lock() {
        mutator(&mut state);
    }
}

pub(crate) fn latest_command_surface_event_sequence_metadata(
) -> CommandSurfaceEventSequenceMetadata {
    command_surface_event_sequence_cache()
        .lock()
        .map(|state| *state)
        .unwrap_or_default()
}

pub(crate) fn note_command_surface_route_resolved() {
    update_command_surface_event_sequences(|state| {
        state.route_events.resolved = state.route_events.resolved.saturating_add(1);
    });
}

pub(crate) fn note_command_surface_route_fallback() {
    update_command_surface_event_sequences(|state| {
        state.route_events.fallback = state.route_events.fallback.saturating_add(1);
    });
}

pub(crate) fn note_command_surface_route_no_target() {
    update_command_surface_event_sequences(|state| {
        state.route_events.no_target = state.route_events.no_target.saturating_add(1);
    });
}

#[cfg(test)]
pub(crate) fn set_command_surface_event_sequence_metadata_for_tests(
    metadata: CommandSurfaceEventSequenceMetadata,
) {
    if let Ok(mut state) = command_surface_event_sequence_cache().lock() {
        *state = metadata;
    }
}

#[cfg(test)]
pub(crate) fn clear_command_surface_event_sequence_metadata() {
    if let Ok(mut state) = command_surface_event_sequence_cache().lock() {
        *state = CommandSurfaceEventSequenceMetadata::default();
    }
}

pub(crate) fn publish_command_surface_semantic_snapshot(
    snapshot: CommandSurfaceSemanticSnapshot,
) {
    if let Ok(mut slot) = command_surface_snapshot_cache().lock() {
        *slot = Some(snapshot);
    }
}

pub(crate) fn latest_command_surface_semantic_snapshot(
) -> Option<CommandSurfaceSemanticSnapshot> {
    command_surface_snapshot_cache()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

pub(crate) fn clear_command_surface_semantic_snapshot() {
    if let Ok(mut slot) = command_surface_snapshot_cache().lock() {
        *slot = None;
    }
    #[cfg(test)]
    clear_command_surface_event_sequence_metadata();
}

#[cfg(test)]
static COMMAND_SURFACE_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn lock_command_surface_snapshot_tests() -> std::sync::MutexGuard<'static, ()> {
    COMMAND_SURFACE_TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("command-surface snapshot test mutex poisoned")
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandBarFocusTarget {
    pane_id: Option<PaneId>,
    node_key: Option<NodeKey>,
}

impl CommandBarFocusTarget {
    pub(crate) fn new(pane_id: Option<PaneId>, node_key: Option<NodeKey>) -> Self {
        Self { pane_id, node_key }
    }

    pub(crate) fn active_pane(self) -> Option<PaneId> {
        self.pane_id
    }

    pub(crate) fn focused_node(self) -> Option<NodeKey> {
        self.node_key
    }
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
    pub command_bar_focus_target: CommandBarFocusTarget,
    pub workbench_layer_state: WorkbenchLayerState,
    pub focused_content_status: &'a FocusedContentStatus,
    pub runtime_focus_state: Option<&'a RuntimeFocusState>,
    pub local_widget_focus: &'a mut Option<LocalFocusTarget>,
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
    pub status_bar_rect: Option<egui::Rect>,
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

fn command_surface_semantic_snapshot(
    graph_app: &GraphBrowserApp,
    command_bar_focus_target: CommandBarFocusTarget,
    local_widget_focus: &Option<LocalFocusTarget>,
    omnibar_search_session: &Option<OmnibarSearchSession>,
    location: &str,
) -> CommandSurfaceSemanticSnapshot {
    let event_sequences = latest_command_surface_event_sequence_metadata();
    let location_focused = matches!(
        local_widget_focus,
        Some(LocalFocusTarget::ToolbarLocation { .. })
    );
    let omnibar_query = omnibar_search_session
        .as_ref()
        .map(|session| session.query.trim().to_string())
        .filter(|query| !query.is_empty())
        .or_else(|| {
            let trimmed = location.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });
    let omnibar_match_count = omnibar_search_session
        .as_ref()
        .map(|session| session.matches.len())
        .unwrap_or(0);
    let omnibar_provider_status = omnibar_search_session
        .as_ref()
        .and_then(|session| provider_status_label(session.provider_mailbox.status));
    let palette_metadata = PaletteSurfaceSemanticMetadata {
        contextual_mode: graph_app.workspace.chrome_ui.command_palette_contextual_mode,
        return_target: graph_app.pending_command_surface_return_target(),
        pending_node_context_target: graph_app.pending_node_context_target(),
        pending_frame_context_target: graph_app.pending_frame_context_target().map(str::to_string),
        context_anchor_present: graph_app.workspace.chrome_ui.context_palette_anchor.is_some(),
    };

    CommandSurfaceSemanticSnapshot {
        command_bar: CommandBarSemanticMetadata {
            active_pane: command_bar_focus_target.active_pane(),
            focused_node: command_bar_focus_target.focused_node(),
            location_focused,
            route_events: event_sequences.route_events,
        },
        omnibar: OmnibarSemanticMetadata {
            active: location_focused || omnibar_search_session.is_some(),
            focused: location_focused,
            query: omnibar_query,
            match_count: omnibar_match_count,
            provider_status: omnibar_provider_status,
            active_pane: command_bar_focus_target.active_pane(),
            focused_node: command_bar_focus_target.focused_node(),
            mailbox_events: event_sequences.omnibar_mailbox_events,
        },
        command_palette: graph_app
            .workspace
            .chrome_ui
            .show_command_palette
            .then_some(palette_metadata.clone()),
        context_palette: graph_app
            .workspace
            .chrome_ui
            .show_context_palette
            .then_some(palette_metadata),
    }
}

pub(super) fn emit_omnibar_provider_mailbox_request_started(query: &str) {
    update_command_surface_event_sequences(|state| {
        state.omnibar_mailbox_events.request_started = state
            .omnibar_mailbox_events
            .request_started
            .saturating_add(1);
    });
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED,
            byte_len: query.trim().len().max(1),
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_applied() {
    update_command_surface_event_sequences(|state| {
        state.omnibar_mailbox_events.applied = state
            .omnibar_mailbox_events
            .applied
            .saturating_add(1);
    });
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
            latency_us: 0,
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_failed() {
    update_command_surface_event_sequences(|state| {
        state.omnibar_mailbox_events.failed = state
            .omnibar_mailbox_events
            .failed
            .saturating_add(1);
    });
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
            byte_len: 1,
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_stale() {
    update_command_surface_event_sequences(|state| {
        state.omnibar_mailbox_events.stale = state
            .omnibar_mailbox_events
            .stale
            .saturating_add(1);
    });
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
    command_bar_focus_target: CommandBarFocusTarget,
) {
    let fullscreen_url = command_bar_focus_target
        .focused_node()
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

// Routing helpers retained for Navigator chrome and tests.

fn enqueue_navigator_view_focus(graph_app: &mut GraphBrowserApp, view_id: GraphViewId) {
    graph_app.enqueue_workbench_intent(WorkbenchIntent::FocusGraphView { view_id });
}

fn enqueue_overview_plane_toggle(graph_app: &mut GraphBrowserApp) {
    graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleOverviewPlane);
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

// ── Left-column Shell-owned controls ──────────────────────────────────
//
// The command bar retains only Shell-owned controls. Navigator, Viewer,
// and Graph controls have been removed per the prototype conformance
// target (2026-04-06). Those controls remain accessible through
// keyboard bindings, the command palette, the action registry, or their
// respective domain-local chrome surfaces.
//
// Removed from the left column (all accessible via alternative surfaces):
//   Navigator view tabs   → Navigator host chrome, with a host-owned fallback
//                            strip when no graph-scope host is currently rendered
//   Wry/Servo compat      → Settings menu or pane-local viewer debug surface
//   Undo / Redo           → keyboard shortcuts (Ctrl+Z / Ctrl+Y), action registry
//   +Node / +Edge / +Tag  → command palette, radial menu, action registry
//   Lens menu             → graph-pane overlay controls
//   Physics menu          → Settings menu, action registry

/// **Shell**-owned controls in the left column: the overview-plane toggle
/// and the command-palette trigger. Both route through `WorkbenchIntent`
/// and are canonical Shell authority.
fn render_command_bar_shell_actions(ui: &mut egui::Ui, graph_app: &mut GraphBrowserApp) {
    let overview_label = if graph_app.graph_view_layout_manager_active() {
        "Overview*"
    } else {
        "Overview"
    };
    let overview_button = ui
        .add(toolbar_button(overview_label))
        .on_hover_text(overview_plane_tooltip(graph_app));
    if overview_button.clicked() {
        enqueue_overview_plane_toggle(graph_app);
    }

    let command_button = ui
        .add(toolbar_button("Cmd"))
        .on_hover_text("Open command palette (F2)");
    if command_button.clicked() {
        toolbar_routing::request_command_palette_toggle(graph_app);
    }
}

/// Renders the left column of the command bar. Contains only Shell-owned
/// controls: the overview toggle and the command-palette trigger.
fn render_command_bar_left_column(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
) {
    render_command_bar_shell_actions(ui, graph_app);
}

/// Renders the right column of the command bar. Contains only Shell-owned
/// controls: Settings menu.
///
/// Removed per prototype conformance target (2026-04-06):
///   Viewer navigation (Back/Forward/Reload/Zoom) → keyboard bindings,
///     action registry; exit path is pane-local viewer chrome.
///   Graph Fit → keyboard binding, action registry.
fn render_command_bar_right_column(
    ui: &mut egui::Ui,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    command_bar_focus_target: CommandBarFocusTarget,
    is_graph_view: bool,
    location_dirty: &mut bool,
    frame_intents: &mut Vec<GraphIntent>,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) {
    render_toolbar_right_controls(
        ui,
        state,
        graph_app,
        window,
        command_bar_focus_target,
        is_graph_view,
        location_dirty,
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
        command_bar_focus_target,
        workbench_layer_state,
        focused_content_status,
        runtime_focus_state,
        local_widget_focus,
        location,
        location_dirty,
        location_submitted,
        focus_location_field_for_search,
        show_clear_data_confirm: _show_clear_data_confirm,
        omnibar_search_session,
        frame_intents,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    } = args;

    if winit_window.fullscreen().is_some() {
        clear_command_surface_semantic_snapshot();
        render_fullscreen_origin_strip(ctx, graph_app, command_bar_focus_target);
        return Output {
            toggle_tile_view_requested: false,
            open_selected_mode_after_submit: None,
            toolbar_visible: false,
            command_bar_rect: None,
            status_bar_rect: None,
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
                        command_bar_focus_target,
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
                        command_bar_focus_target,
                        is_graph_view,
                        location_dirty,
                        frame_intents,
                        #[cfg(feature = "diagnostics")]
                        diagnostics_state,
                    );
                });
            });
        });

    publish_command_surface_semantic_snapshot(command_surface_semantic_snapshot(
        graph_app,
        command_bar_focus_target,
        local_widget_focus,
        omnibar_search_session,
        location,
    ));

    #[cfg(feature = "diagnostics")]
    diagnostics_state.tick_drain();
    #[cfg(feature = "diagnostics")]
    let ambient_diagnostics_attention = diagnostics_state.ambient_attention_summary();

    let status_bar_rect = render_shell_status_bar(
        ctx,
        workbench_layer_state,
        focused_content_status,
        runtime_focus_state,
        #[cfg(feature = "diagnostics")]
        ambient_diagnostics_attention.as_ref(),
    );

    Output {
        toggle_tile_view_requested,
        open_selected_mode_after_submit,
        toolbar_visible: true,
        command_bar_rect: Some(command_bar_response.response.rect),
        status_bar_rect: Some(status_bar_rect),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
        CommandSurfaceSemanticSnapshot, OmnibarMailboxEventSequenceMetadata,
        OmnibarSemanticMetadata, PaletteSurfaceSemanticMetadata,
        clear_command_surface_semantic_snapshot,
        enqueue_navigator_view_focus, enqueue_overview_plane_toggle,
        emit_omnibar_provider_mailbox_applied, emit_omnibar_provider_mailbox_failed,
        emit_omnibar_provider_mailbox_request_started, emit_omnibar_provider_mailbox_stale,
        latest_command_surface_semantic_snapshot, publish_command_surface_semantic_snapshot,
        render_shell_status_bar, TOOLBAR_HEIGHT,
    };
    use crate::app::{GraphBrowserApp, GraphViewId, WorkbenchIntent};
    use crate::shell::desktop::runtime::diagnostics::{
        DiagnosticEvent, install_global_sender,
    };
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED,
        CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE,
    };
    use crate::shell::desktop::ui::gui_state::FocusedContentStatus;
    use crate::shell::desktop::ui::toolbar_routing;
    use crate::shell::desktop::ui::workbench_host::WorkbenchLayerState;

    #[test]
    fn navigator_view_tabs_enqueue_workbench_focus_intent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();

        enqueue_navigator_view_focus(&mut app, view_id);

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::FocusGraphView { view_id: focused }] if *focused == view_id
        ));
    }

    #[test]
    fn overview_button_enqueue_workbench_toggle_intent() {
        let mut app = GraphBrowserApp::new_for_testing();

        enqueue_overview_plane_toggle(&mut app);

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleOverviewPlane]
        ));
    }

    #[test]
    fn command_bar_command_palette_request_emits_diagnostic() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);
        let mut app = GraphBrowserApp::new_for_testing();

        toolbar_routing::request_command_palette_toggle(&mut app);

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED
            )),
            "expected command bar dispatch diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageReceivedStructured { channel_id, fields, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED
                        && fields.iter().any(|field| field.name == "route_detail" && field.value == "intent_enqueued")
            )),
            "expected generic command-surface resolved diagnostic; got: {emitted:?}"
        );
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleCommandPalette]
        ));
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

    #[test]
    fn command_surface_semantic_snapshot_cache_round_trips() {
        let _guard = super::lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        let snapshot = CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: true,
                focused: true,
                query: Some("rust async".to_string()),
                match_count: 3,
                provider_status: Some("Suggestions: loading...".to_string()),
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata {
                contextual_mode: false,
                return_target: None,
                pending_node_context_target: None,
                pending_frame_context_target: None,
                context_anchor_present: false,
            }),
            context_palette: None,
        };

        publish_command_surface_semantic_snapshot(snapshot.clone());

        assert_eq!(latest_command_surface_semantic_snapshot(), Some(snapshot));

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn shell_chrome_composes_command_and_status_bars_in_same_frame() {
        let ctx = egui::Context::default();
        let mut command_bar_rect = None;
        let mut status_bar_rect = None;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let response = egui::TopBottomPanel::top("shell_command_bar_test")
                .exact_height(TOOLBAR_HEIGHT)
                .show(ctx, |ui| {
                    ui.label("Command bar");
                });
            command_bar_rect = Some(response.response.rect);
            status_bar_rect = Some(render_shell_status_bar(
                ctx,
                WorkbenchLayerState::GraphOnly,
                &FocusedContentStatus::unavailable(None, None),
                None,
                #[cfg(feature = "diagnostics")]
                None,
            ));
        });

        let command_bar_rect = command_bar_rect.expect("command bar should render a rect");
        let status_bar_rect = status_bar_rect.expect("status bar should render a rect");
        assert!(command_bar_rect.height() > 0.0);
        assert!(status_bar_rect.height() > 0.0);
        assert!(command_bar_rect.min.y <= status_bar_rect.min.y);
        assert!(command_bar_rect.max.y <= status_bar_rect.max.y);
    }
}

