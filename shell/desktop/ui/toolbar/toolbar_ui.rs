/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Key, Modifiers, TopBottomPanel, Vec2};
use egui_tiles::Tree;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use winit::window::Window;

use crate::shell::desktop::runtime::control_panel::{ControlPanel, HostRequestPoll};
pub(crate) use crate::shell::desktop::ui::omnibar_state::{
    HistoricalNodeMatch, OmnibarMatch, OmnibarSearchMode, OmnibarSearchSession, OmnibarSessionKind,
    ProviderSuggestionError, ProviderSuggestionFetchOutcome, ProviderSuggestionMailbox,
    ProviderSuggestionStatus, SearchProviderKind,
};
use crate::shell::desktop::runtime::protocols::router::{self, OutboundFetchError};
use crate::shell::desktop::ui::gui_state::{
    FocusedContentStatus, LocalFocusTarget, RuntimeFocusState, ToolbarAuthorityMut,
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
#[path = "toolbar_settings_menu.rs"]
mod toolbar_settings_menu;
#[path = "toolbar_status_bar.rs"]
mod toolbar_status_bar;
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
use self::toolbar_status_bar::render_shell_status_bar;
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
    CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED, CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
    CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED, CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE,
    input::action_id, phase2_binding_display_labels_for_action,
};
use crate::shell::desktop::ui::navigator_context::NavigatorContextProjection;
use crate::shell::desktop::workbench::tile_kind::TileKind;

const WORKSPACE_PIN_NAME: &str = "workspace:pin:space";
const OMNIBAR_PROVIDER_MIN_QUERY_LEN: usize = 2;
const OMNIBAR_CONNECTED_NON_AT_CAP: usize = 8;
const OMNIBAR_GLOBAL_NODES_FALLBACK_CAP: usize = 3;
const OMNIBAR_GLOBAL_TABS_FALLBACK_CAP: usize = 3;
/// Test-only fallback for the top chrome bar height. Live code reads
/// `graph_app.workspace.chrome_ui.toolbar_height_dp` (settings-backed);
/// this literal is kept for unit tests that construct an egui panel
/// without a `GraphBrowserApp`.
#[cfg(test)]
const TOOLBAR_HEIGHT: f32 = 40.0;

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

// Telemetry shapes and the consolidated sink live in
// `shell/desktop/ui/command_surface_telemetry.rs`. Re-exported here so
// existing `pub(crate) use crate::shell::desktop::ui::toolbar::toolbar_ui::{...}`
// imports in tests and `ux_probes` keep resolving unchanged.
pub(crate) use crate::shell::desktop::ui::command_surface_telemetry::{
    CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
    CommandSurfaceEventSequenceMetadata, CommandSurfaceSemanticSnapshot, CommandSurfaceTelemetry,
    OmnibarMailboxEventSequenceMetadata, OmnibarSemanticMetadata, PaletteSurfaceSemanticMetadata,
};

pub(crate) fn latest_command_surface_event_sequence_metadata() -> CommandSurfaceEventSequenceMetadata
{
    CommandSurfaceTelemetry::global().latest_event_sequence_metadata()
}

pub(crate) fn note_command_surface_route_resolved() {
    CommandSurfaceTelemetry::global().note_route_resolved();
}

pub(crate) fn note_command_surface_route_fallback() {
    CommandSurfaceTelemetry::global().note_route_fallback();
}

pub(crate) fn note_command_surface_route_no_target() {
    CommandSurfaceTelemetry::global().note_route_no_target();
}

#[cfg(test)]
pub(crate) fn set_command_surface_event_sequence_metadata_for_tests(
    metadata: CommandSurfaceEventSequenceMetadata,
) {
    CommandSurfaceTelemetry::global().set_event_sequence_metadata_for_tests(metadata);
}

#[cfg(test)]
pub(crate) fn clear_command_surface_event_sequence_metadata() {
    CommandSurfaceTelemetry::global().clear_event_sequence_metadata();
}

pub(crate) fn publish_command_surface_semantic_snapshot(snapshot: CommandSurfaceSemanticSnapshot) {
    CommandSurfaceTelemetry::global().publish_snapshot(snapshot);
}

pub(crate) fn latest_command_surface_semantic_snapshot() -> Option<CommandSurfaceSemanticSnapshot> {
    CommandSurfaceTelemetry::global().latest_snapshot()
}

pub(crate) fn clear_command_surface_semantic_snapshot() {
    CommandSurfaceTelemetry::global().clear_snapshot();
}

#[cfg(test)]
pub(crate) fn lock_command_surface_snapshot_tests() -> std::sync::MutexGuard<'static, ()> {
    CommandSurfaceTelemetry::global().lock_tests()
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
    /// Host-facing mutation handle for the toolbar's editable surface,
    /// clear-data arm flag, and omnibar session. Destructured into
    /// individual refs inside `render_toolbar_ui` so sub-widgets keep
    /// their existing raw-ref signatures.
    pub toolbar_authority: ToolbarAuthorityMut<'a>,
    pub focus_location_field_for_search: bool,
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
        contextual_mode: graph_app
            .workspace
            .chrome_ui
            .command_palette_contextual_mode,
        return_target: graph_app.pending_command_surface_return_target(),
        pending_node_context_target: graph_app.pending_node_context_target(),
        pending_frame_context_target: graph_app.pending_frame_context_target().map(str::to_string),
        context_anchor_present: graph_app
            .workspace
            .chrome_ui
            .context_palette_anchor
            .is_some(),
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
    CommandSurfaceTelemetry::global().note_omnibar_mailbox_request_started();
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED,
            byte_len: query.trim().len().max(1),
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_applied() {
    CommandSurfaceTelemetry::global().note_omnibar_mailbox_applied();
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
            latency_us: 0,
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_failed() {
    CommandSurfaceTelemetry::global().note_omnibar_mailbox_failed();
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED,
            byte_len: 1,
        },
    );
}

pub(super) fn emit_omnibar_provider_mailbox_stale() {
    CommandSurfaceTelemetry::global().note_omnibar_mailbox_stale();
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
    let panel_bg = crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
        graph_app.default_registry_theme_id(),
    )
    .tokens
    .workbench_panel_background;
    let frame_fill = egui::Color32::from_rgba_unmultiplied(
        panel_bg.r(),
        panel_bg.g(),
        panel_bg.b(),
        220,
    );
    let frame = egui::Frame::default().fill(frame_fill).inner_margin(4.0);
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
fn render_command_bar_left_column(ui: &mut egui::Ui, graph_app: &mut GraphBrowserApp) {
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
        toolbar_authority,
        focus_location_field_for_search,
        frame_intents,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    } = args;

    // Destructure the toolbar-authority bundle into the raw refs the
    // existing sub-widget call surface expects. The bundle is the
    // host-facing shape (what `ToolbarDialogPhaseArgs` passes in); the
    // individual refs are the widget-internal shape (what
    // `render_location_search_panel` and `render_command_bar_right_column`
    // still consume). `_show_clear_data_confirm` is threaded through
    // `publish_command_surface_semantic_snapshot` but is otherwise read
    // via the bundle's own accessors in follow-on slices.
    let ToolbarAuthorityMut {
        editable,
        show_clear_data_confirm: _show_clear_data_confirm,
        omnibar_search_session,
    } = toolbar_authority;
    let location = &mut editable.location;
    let location_dirty = &mut editable.location_dirty;
    let location_submitted = &mut editable.location_submitted;

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
    let toolbar_height = graph_app.workspace.chrome_ui.toolbar_height_dp;
    let frame = egui::Frame::default()
        .fill(ctx.style().visuals.window_fill)
        .inner_margin(4.0);
    let command_bar_response = TopBottomPanel::top("shell_command_bar")
        .frame(frame)
        .exact_height(toolbar_height)
        .show(ctx, |ui| {
            ui.columns(3, |columns| {
                columns[0].horizontal_wrapped(|ui| {
                    render_command_bar_left_column(ui, graph_app);
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
        OmnibarSemanticMetadata, PaletteSurfaceSemanticMetadata, TOOLBAR_HEIGHT,
        clear_command_surface_semantic_snapshot, emit_omnibar_provider_mailbox_applied,
        emit_omnibar_provider_mailbox_failed, emit_omnibar_provider_mailbox_request_started,
        emit_omnibar_provider_mailbox_stale, enqueue_navigator_view_focus,
        enqueue_overview_plane_toggle, latest_command_surface_semantic_snapshot,
        publish_command_surface_semantic_snapshot, render_shell_status_bar,
    };
    use crate::app::{GraphBrowserApp, GraphViewId, WorkbenchIntent};
    use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, install_global_sender};
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED, CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED,
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
