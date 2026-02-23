/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crossbeam_channel::Receiver;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Key, Modifiers, Slider, TopBottomPanel, Vec2, WidgetInfo, WidgetType};
use egui_tiles::Tree;
use euclid::default::Point2D;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::thread;
use std::time::{Duration, Instant};
use winit::window::Window;

use super::tile_grouping;
use super::protocols::router::{self, OutboundFetchError};
use super::toolbar_routing::{self, ToolbarNavAction, ToolbarOpenMode};
mod toolbar_omnibar;
mod toolbar_location_panel;
use self::toolbar_location_panel::render_location_search_panel;
use self::toolbar_omnibar::{
    apply_omnibar_match, dedupe_matches_in_order, default_search_provider_from_searchpage,
    graph_center_for_new_node, non_at_primary_matches_for_scope, omnibar_match_label,
    omnibar_match_signifier, omnibar_matches_for_query, non_at_global_fallback_matches,
    non_at_matches_for_settings, parse_omnibar_search_query, parse_provider_search_query,
    searchpage_template_for_provider, spawn_provider_suggestion_request,
};
use crate::app::{
    CommandPaletteShortcut, GraphBrowserApp, GraphIntent, HelpPanelShortcut, LassoMouseBinding,
    OmnibarNonAtOrderPreset, OmnibarPreferredScope, PendingTileOpenMode, RadialMenuShortcut,
    ToastAnchorPreference,
};
use super::selection_range::inclusive_index_range;
use crate::desktop::tile_kind::TileKind;
use crate::graph::NodeKey;
use crate::running_app_state::{RunningAppState, UserInterfaceCommand};
use crate::search::{fuzzy_match_items, fuzzy_match_node_keys};
use crate::window::EmbedderWindow;

const WORKSPACE_PIN_NAME: &str = "workspace:pin:space";
const OMNIBAR_DROPDOWN_MAX_ROWS: usize = 8;
const OMNIBAR_PROVIDER_MIN_QUERY_LEN: usize = 2;
const OMNIBAR_CONNECTED_NON_AT_CAP: usize = 8;
const OMNIBAR_GLOBAL_NODES_FALLBACK_CAP: usize = 3;
const OMNIBAR_GLOBAL_TABS_FALLBACK_CAP: usize = 3;
const OMNIBAR_PROVIDER_DEBOUNCE_MS: u64 = 140;

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OmnibarMatch {
    Node(NodeKey),
    NodeUrl(String),
    SearchQuery {
        query: String,
        provider: SearchProviderKind,
    },
    Edge {
        from: NodeKey,
        to: NodeKey,
    },
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
    provider_rx: Option<Receiver<ProviderSuggestionFetchOutcome>>,
    provider_debounce_deadline: Option<Instant>,
    provider_status: ProviderSuggestionStatus,
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

pub(crate) struct ToolbarUiInput<'a> {
    pub ctx: &'a egui::Context,
    pub winit_window: &'a Window,
    pub state: &'a RunningAppState,
    pub graph_app: &'a mut GraphBrowserApp,
    pub window: &'a EmbedderWindow,
    pub tiles_tree: &'a Tree<TileKind>,
    pub focused_toolbar_node: Option<NodeKey>,
    pub has_webview_tiles: bool,
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
    pub diagnostics_state: &'a mut crate::desktop::diagnostics::DiagnosticsState,
}

pub(crate) struct ToolbarUiOutput {
    pub toggle_tile_view_requested: bool,
    pub open_selected_mode_after_submit: Option<ToolbarOpenMode>,
    pub toolbar_visible: bool,
}

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

fn lasso_binding_label(binding: LassoMouseBinding) -> &'static str {
    match binding {
        LassoMouseBinding::RightDrag => "Right Drag (Default)",
        LassoMouseBinding::ShiftLeftDrag => "Shift + Left Drag",
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
        },
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => {
            "Provider -> Contextual -> Global"
        },
    }
}

fn provider_status_label(status: ProviderSuggestionStatus) -> Option<String> {
    match status {
        ProviderSuggestionStatus::Idle => None,
        ProviderSuggestionStatus::Loading => Some("Suggestions: loading...".to_string()),
        ProviderSuggestionStatus::Ready => None,
        ProviderSuggestionStatus::Failed(ProviderSuggestionError::Network) => {
            Some("Suggestions unavailable: network error".to_string())
        },
        ProviderSuggestionStatus::Failed(ProviderSuggestionError::HttpStatus(code)) => {
            Some(format!("Suggestions unavailable: provider http {code}"))
        },
        ProviderSuggestionStatus::Failed(ProviderSuggestionError::Parse) => {
            Some("Suggestions unavailable: response parse error".to_string())
        },
    }
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

fn workspace_pin_name_for_node(node: NodeKey, graph_app: &GraphBrowserApp) -> Option<String> {
    graph_app
        .graph
        .get_node(node)
        .map(|n| format!("workspace:pin:pane:{}", n.id))
}

fn render_settings_menu(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    state: &RunningAppState,
    frame_intents: &mut Vec<GraphIntent>,
    location_dirty: &mut bool,
    window: &EmbedderWindow,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::desktop::diagnostics::DiagnosticsState,
) {
    if ui.button("Open Persistence Hub").clicked() {
        graph_app.show_persistence_panel = true;
        ui.close();
    }
    if ui
        .button(if graph_app.show_physics_panel {
            "Hide Physics Panel"
        } else {
            "Show Physics Panel"
        })
        .clicked()
    {
        frame_intents.push(GraphIntent::TogglePhysicsPanel);
        ui.close();
    }
    if ui
        .button(if graph_app.show_help_panel {
            "Hide Help Panel"
        } else {
            "Show Help Panel"
        })
        .clicked()
    {
        frame_intents.push(GraphIntent::ToggleHelpPanel);
        ui.close();
    }
    if ui
        .button(if graph_app.show_traversal_history_panel {
            "Hide History Panel"
        } else {
            "Show History Panel"
        })
        .clicked()
    {
        frame_intents.push(GraphIntent::ToggleTraversalHistoryPanel);
        ui.close();
    }
    ui.separator();
    ui.label(format!(
        "Toasts: {}",
        toast_anchor_label(graph_app.toast_anchor_preference)
    ));
    for anchor in [
        ToastAnchorPreference::BottomRight,
        ToastAnchorPreference::BottomLeft,
        ToastAnchorPreference::TopRight,
        ToastAnchorPreference::TopLeft,
    ] {
        if ui
            .selectable_label(
                graph_app.toast_anchor_preference == anchor,
                toast_anchor_label(anchor),
            )
            .clicked()
        {
            graph_app.set_toast_anchor_preference(anchor);
        }
    }
    ui.separator();
    ui.label("Graph Zoom");
    let mut zoom_impulse = graph_app.scroll_zoom_impulse_scale;
    if ui
        .add(
            Slider::new(
                &mut zoom_impulse,
                GraphBrowserApp::MIN_SCROLL_ZOOM_IMPULSE_SCALE
                    ..=GraphBrowserApp::MAX_SCROLL_ZOOM_IMPULSE_SCALE,
            )
            .text("Inertia Impulse"),
        )
        .changed()
    {
        graph_app.set_scroll_zoom_impulse_scale(zoom_impulse);
    }
    let mut zoom_damping = graph_app.scroll_zoom_inertia_damping;
    if ui
        .add(
            Slider::new(
                &mut zoom_damping,
                GraphBrowserApp::MIN_SCROLL_ZOOM_INERTIA_DAMPING
                    ..=GraphBrowserApp::MAX_SCROLL_ZOOM_INERTIA_DAMPING,
            )
            .text("Inertia Damping"),
        )
        .changed()
    {
        graph_app.set_scroll_zoom_inertia_damping(zoom_damping);
    }
    let mut zoom_min_abs = graph_app.scroll_zoom_inertia_min_abs;
    if ui
        .add(
            Slider::new(
                &mut zoom_min_abs,
                GraphBrowserApp::MIN_SCROLL_ZOOM_INERTIA_MIN_ABS
                    ..=GraphBrowserApp::MAX_SCROLL_ZOOM_INERTIA_MIN_ABS,
            )
            .text("Inertia Stop Threshold"),
        )
        .changed()
    {
        graph_app.set_scroll_zoom_inertia_min_abs(zoom_min_abs);
    }
    ui.separator();
    ui.label("Input");
    ui.label(format!(
        "Lasso: {}",
        lasso_binding_label(graph_app.lasso_mouse_binding)
    ));
    for binding in [LassoMouseBinding::RightDrag, LassoMouseBinding::ShiftLeftDrag] {
        if ui
            .selectable_label(
                graph_app.lasso_mouse_binding == binding,
                lasso_binding_label(binding),
            )
            .clicked()
        {
            graph_app.set_lasso_mouse_binding(binding);
        }
    }
    ui.label(format!(
        "Command Palette: {}",
        command_palette_shortcut_label(graph_app.command_palette_shortcut)
    ));
    for shortcut in [CommandPaletteShortcut::F2, CommandPaletteShortcut::CtrlK] {
        if ui
            .selectable_label(
                graph_app.command_palette_shortcut == shortcut,
                command_palette_shortcut_label(shortcut),
            )
            .clicked()
        {
            graph_app.set_command_palette_shortcut(shortcut);
        }
    }
    ui.label(format!(
        "Help: {}",
        help_shortcut_label(graph_app.help_panel_shortcut)
    ));
    for shortcut in [HelpPanelShortcut::F1OrQuestion, HelpPanelShortcut::H] {
        if ui
            .selectable_label(
                graph_app.help_panel_shortcut == shortcut,
                help_shortcut_label(shortcut),
            )
            .clicked()
        {
            graph_app.set_help_panel_shortcut(shortcut);
        }
    }
    ui.label(format!(
        "Radial: {}",
        radial_shortcut_label(graph_app.radial_menu_shortcut)
    ));
    for shortcut in [RadialMenuShortcut::F3, RadialMenuShortcut::R] {
        if ui
            .selectable_label(
                graph_app.radial_menu_shortcut == shortcut,
                radial_shortcut_label(shortcut),
            )
            .clicked()
        {
            graph_app.set_radial_menu_shortcut(shortcut);
        }
    }
    ui.separator();
    ui.label("Omnibar");
    ui.label(format!(
        "Preferred Scope: {}",
        omnibar_preferred_scope_label(graph_app.omnibar_preferred_scope)
    ));
    for scope in [
        OmnibarPreferredScope::Auto,
        OmnibarPreferredScope::LocalTabs,
        OmnibarPreferredScope::ConnectedNodes,
        OmnibarPreferredScope::ProviderDefault,
        OmnibarPreferredScope::GlobalNodes,
        OmnibarPreferredScope::GlobalTabs,
    ] {
        if ui
            .selectable_label(
                graph_app.omnibar_preferred_scope == scope,
                omnibar_preferred_scope_label(scope),
            )
            .clicked()
        {
            graph_app.set_omnibar_preferred_scope(scope);
        }
    }
    ui.label(format!(
        "Non-@ Order: {}",
        omnibar_non_at_order_label(graph_app.omnibar_non_at_order)
    ));
    for order in [
        OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal,
    ] {
        if ui
            .selectable_label(
                graph_app.omnibar_non_at_order == order,
                omnibar_non_at_order_label(order),
            )
            .clicked()
        {
            graph_app.set_omnibar_non_at_order(order);
        }
    }
    ui.separator();
    ui.label("Preferences");
    if ui.button("Open Preferences Page").clicked() {
        request_open_settings_page(graph_app, frame_intents, "servo:preferences");
        ui.close();
    }
    if ui.button("Open Experimental Preferences").clicked() {
        request_open_settings_page(graph_app, frame_intents, "servo:experimental-preferences");
        ui.close();
    }
    let mut experimental_preferences_enabled = state.experimental_preferences_enabled();
    let prefs_toggle = ui
        .toggle_value(
            &mut experimental_preferences_enabled,
            "Experimental Preferences",
        )
        .on_hover_text("Enable experimental prefs");
    if prefs_toggle.clicked() {
        state.set_experimental_preferences_enabled(experimental_preferences_enabled);
        *location_dirty = false;
        window.queue_user_interface_command(UserInterfaceCommand::ReloadAll);
    }

    ui.separator();
    ui.label("Registry Defaults");

    let mut lens_id = graph_app
        .default_registry_lens_id()
        .unwrap_or_default()
        .to_string();
    if ui
        .horizontal(|ui| {
            ui.label("Lens ID");
            ui.text_edit_singleline(&mut lens_id)
        })
        .inner
        .changed()
    {
        let value = lens_id.trim();
        graph_app.set_default_registry_lens_id((!value.is_empty()).then_some(value));
    }

    let mut physics_id = graph_app
        .default_registry_physics_id()
        .unwrap_or_default()
        .to_string();
    if ui
        .horizontal(|ui| {
            ui.label("Physics ID");
            ui.text_edit_singleline(&mut physics_id)
        })
        .inner
        .changed()
    {
        let value = physics_id.trim();
        graph_app.set_default_registry_physics_id((!value.is_empty()).then_some(value));
    }

    let mut layout_id = graph_app
        .default_registry_layout_id()
        .unwrap_or_default()
        .to_string();
    if ui
        .horizontal(|ui| {
            ui.label("Layout ID");
            ui.text_edit_singleline(&mut layout_id)
        })
        .inner
        .changed()
    {
        let value = layout_id.trim();
        graph_app.set_default_registry_layout_id((!value.is_empty()).then_some(value));
    }

    let mut theme_id = graph_app
        .default_registry_theme_id()
        .unwrap_or_default()
        .to_string();
    if ui
        .horizontal(|ui| {
            ui.label("Theme ID");
            ui.text_edit_singleline(&mut theme_id)
        })
        .inner
        .changed()
    {
        let value = theme_id.trim();
        graph_app.set_default_registry_theme_id((!value.is_empty()).then_some(value));
    }

    #[cfg(feature = "diagnostics")]
    {
        ui.separator();
        ui.label("Diagnostics");
        if ui.button("Export Diagnostic Snapshot (JSON)").clicked() {
            match diagnostics_state.export_snapshot_json() {
                Ok(path) => log::info!("Diagnostics JSON exported: {}", path.display()),
                Err(err) => log::warn!("Diagnostics JSON export failed: {err}"),
            }
            ui.close();
        }
        if ui.button("Export Diagnostic Snapshot (SVG)").clicked() {
            match diagnostics_state.export_snapshot_svg() {
                Ok(path) => log::info!("Diagnostics SVG exported: {}", path.display()),
                Err(err) => log::warn!("Diagnostics SVG export failed: {err}"),
            }
            ui.close();
        }
    }
}

fn render_workspace_pin_controls(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    has_webview_tiles: bool,
    focused_pane_pin_name: Option<&str>,
    persisted_workspace_names: &HashSet<String>,
) {
    if !has_webview_tiles {
        return;
    }

    if let Some(pane_pin_name) = focused_pane_pin_name {
        let pane_is_pinned = persisted_workspace_names.contains(pane_pin_name);
        let pane_pin_label = if pane_is_pinned { "P-" } else { "P+" };
        let pane_pin_button = ui.add(toolbar_button(pane_pin_label)).on_hover_text(
            if pane_is_pinned {
                "Unpin focused pane workspace snapshot"
            } else {
                "Pin focused pane workspace snapshot"
            },
        );
        if pane_pin_button.clicked() {
            if pane_is_pinned {
                if let Err(e) = graph_app.delete_workspace_layout(pane_pin_name) {
                    log::warn!(
                        "Failed to unpin focused pane workspace '{pane_pin_name}': {e}"
                    );
                }
            } else {
                graph_app.request_save_workspace_snapshot_named(pane_pin_name.to_string());
            }
        }

        let pane_recall_button = ui
            .add_enabled(pane_is_pinned, toolbar_button("PR"))
            .on_hover_text("Recall focused pane pinned workspace");
        if pane_recall_button.clicked() {
            graph_app.request_restore_workspace_snapshot_named(pane_pin_name.to_string());
        }
    }

    let space_is_pinned = persisted_workspace_names.contains(WORKSPACE_PIN_NAME);
    let space_pin_label = if space_is_pinned { "W-" } else { "W+" };
    let space_pin_button = ui.add(toolbar_button(space_pin_label)).on_hover_text(
        if space_is_pinned {
            "Unpin current workspace snapshot"
        } else {
            "Pin current workspace snapshot"
        },
    );
    if space_pin_button.clicked() {
        if space_is_pinned {
            if let Err(e) = graph_app.delete_workspace_layout(WORKSPACE_PIN_NAME) {
                log::warn!("Failed to unpin workspace snapshot '{WORKSPACE_PIN_NAME}': {e}");
            }
        } else {
            graph_app.request_save_workspace_snapshot_named(WORKSPACE_PIN_NAME.to_string());
        }
    }

    let space_recall_button = ui
        .add_enabled(space_is_pinned, toolbar_button("WR"))
        .on_hover_text("Recall pinned workspace snapshot");
    if space_recall_button.clicked() {
        graph_app.request_restore_workspace_snapshot_named(WORKSPACE_PIN_NAME.to_string());
    }
}

fn render_navigation_buttons(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    focused_toolbar_node: Option<NodeKey>,
    can_go_back: bool,
    can_go_forward: bool,
    location_dirty: &mut bool,
) {
    let back_button = ui.add_enabled(can_go_back, toolbar_button("<"));
    back_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Back".into());
        info
    });
    if back_button.clicked() {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Back,
        );
    }

    let forward_button = ui.add_enabled(can_go_forward, toolbar_button(">"));
    forward_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Forward".into());
        info
    });
    if forward_button.clicked() {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Forward,
        );
    }

    let reload_button = ui.add(toolbar_button("R"));
    reload_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Reload".into());
        info
    });
    if reload_button.clicked() {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Reload,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_toolbar_right_controls(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &Tree<TileKind>,
    focused_toolbar_node: Option<NodeKey>,
    has_webview_tiles: bool,
    is_graph_view: bool,
    location: &mut String,
    location_dirty: &mut bool,
    location_submitted: &mut bool,
    focus_location_field_for_search: bool,
    show_clear_data_confirm: &mut bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    frame_intents: &mut Vec<GraphIntent>,
    focused_pane_pin_name: Option<&str>,
    persisted_workspace_names: &HashSet<String>,
    toggle_tile_view_requested: &mut bool,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::desktop::diagnostics::DiagnosticsState,
) {
    ui.menu_button("Settings", |ui| {
        render_settings_menu(
            ui,
            graph_app,
            state,
            frame_intents,
            location_dirty,
            window,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        );
    });

    let (view_icon, view_tooltip) = if has_webview_tiles {
        ("Graph", "Switch to Graph View")
    } else {
        ("Detail", "Switch to Detail View")
    };
    let view_toggle_button = ui
        .add(toolbar_button(view_icon))
        .on_hover_text(view_tooltip);
    view_toggle_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Toggle View".into());
        info
    });
    if view_toggle_button.clicked() {
        *toggle_tile_view_requested = true;
    }

    let clear_data_button = ui
        .add(toolbar_button("Clr"))
        .on_hover_text("Clear graph and saved data");
    clear_data_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Clear graph and saved data".into());
        info
    });
    if clear_data_button.clicked() {
        *show_clear_data_confirm = true;
    }

    let command_button = ui
        .add(toolbar_button("Cmd"))
        .on_hover_text("Open command palette (F2)");
    if command_button.clicked() {
        frame_intents.push(GraphIntent::ToggleCommandPalette);
    }

    render_workspace_pin_controls(
        ui,
        graph_app,
        has_webview_tiles,
        focused_pane_pin_name,
        persisted_workspace_names,
    );

    render_location_search_panel(
        ui,
        ctx,
        state,
        graph_app,
        window,
        tiles_tree,
        focused_toolbar_node,
        has_webview_tiles,
        is_graph_view,
        location,
        location_dirty,
        location_submitted,
        focus_location_field_for_search,
        omnibar_search_session,
        frame_intents,
        open_selected_mode_after_submit,
    );
}

fn render_fullscreen_origin_strip(
    ctx: &egui::Context,
    graph_app: &GraphBrowserApp,
    focused_toolbar_node: Option<NodeKey>,
) {
    let fullscreen_url = focused_toolbar_node
        .and_then(|key| graph_app.graph.get_node(key).map(|node| node.url.clone()))
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

pub(crate) fn render_toolbar_ui(args: ToolbarUiInput<'_>) -> ToolbarUiOutput {
    let ToolbarUiInput {
        ctx,
        winit_window,
        state,
        graph_app,
        window,
        tiles_tree,
        focused_toolbar_node,
        has_webview_tiles,
        can_go_back,
        can_go_forward,
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
        return ToolbarUiOutput {
            toggle_tile_view_requested: false,
            open_selected_mode_after_submit: None,
            toolbar_visible: false,
        };
    }

    let mut toggle_tile_view_requested = false;
    let mut open_selected_mode_after_submit = None;
    let is_graph_view = !has_webview_tiles;
    let persisted_workspace_names: HashSet<String> = graph_app
        .list_workspace_layout_names()
        .into_iter()
        .collect();
    let focused_pane_pin_name =
        focused_toolbar_node.and_then(|node| workspace_pin_name_for_node(node, graph_app));

    let frame = egui::Frame::default()
        .fill(ctx.style().visuals.window_fill)
        .inner_margin(4.0);
    TopBottomPanel::top("toolbar").frame(frame).show(ctx, |ui| {
        ui.allocate_ui_with_layout(
            ui.available_size(),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                render_navigation_buttons(
                    ui,
                    graph_app,
                    window,
                    focused_toolbar_node,
                    can_go_back,
                    can_go_forward,
                    location_dirty,
                );
                ui.add_space(2.0);

                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        render_toolbar_right_controls(
                            ui,
                            ctx,
                            state,
                            graph_app,
                            window,
                            tiles_tree,
                            focused_toolbar_node,
                            has_webview_tiles,
                            is_graph_view,
                            location,
                            location_dirty,
                            location_submitted,
                            focus_location_field_for_search,
                            show_clear_data_confirm,
                            omnibar_search_session,
                            frame_intents,
                            focused_pane_pin_name.as_deref(),
                            &persisted_workspace_names,
                            &mut toggle_tile_view_requested,
                            &mut open_selected_mode_after_submit,
                            #[cfg(feature = "diagnostics")]
                            diagnostics_state,
                        );
                    },
                );
            },
        );
    });

    ToolbarUiOutput {
        toggle_tile_view_requested,
        open_selected_mode_after_submit,
        toolbar_visible: true,
    }
}
