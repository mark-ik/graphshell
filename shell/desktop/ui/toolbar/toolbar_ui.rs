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
use std::thread;
use std::time::{Duration, Instant};
use winit::window::Window;

use crate::shell::desktop::runtime::protocols::router::{self, OutboundFetchError};
use crate::shell::desktop::ui::gui_state::FocusedContentStatus;
use crate::shell::desktop::ui::gui_state::LocalFocusTarget;
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::ui::workbench_sidebar::WorkbenchLayerState;
use crate::shell::desktop::workbench::pane_model::{PaneId, ViewerId};
use crate::shell::desktop::workbench::tile_grouping;
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
    TagPanelState, ToastAnchorPreference, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::services::search::{fuzzy_match_items, fuzzy_match_node_keys};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::registries::lens::{LENS_ID_DEFAULT, LENS_ID_SEMANTIC_OVERLAY};
use crate::shell::desktop::runtime::registries::physics_profile::{
    PHYSICS_PROFILE_GAS, PHYSICS_PROFILE_LIQUID, PHYSICS_PROFILE_SOLID,
};
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

pub(crate) struct Input<'a> {
    pub ctx: &'a egui::Context,
    pub winit_window: &'a Window,
    pub state: &'a RunningAppState,
    pub graph_app: &'a mut GraphBrowserApp,
    pub window: &'a EmbedderWindow,
    pub tiles_tree: &'a Tree<TileKind>,
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
                .map(|node| node.url.clone())
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

fn render_graph_view_tabs(
    ui: &mut egui::Ui,
    graph_app: &GraphBrowserApp,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let focused = graph_app.workspace.graph_runtime.focused_view;
    let mut views: Vec<(GraphViewId, String)> = graph_app
        .workspace
        .graph_runtime
        .views
        .iter()
        .map(|(id, view)| {
            let name = view.name.trim().to_string();
            let label = if name.is_empty() {
                "Graph".to_string()
            } else {
                name
            };
            (*id, label)
        })
        .collect();
    // Stable order so tabs don't shuffle on each frame.
    views.sort_by_key(|(id, _)| id.as_uuid());

    if views.len() <= 1 {
        // Single view: show plain label, no tab strip needed.
        let label = views
            .first()
            .map(|(_, name)| format!("View: {name}"))
            .unwrap_or_else(|| "View: Graph".to_string());
        ui.label(label);
    } else {
        for (view_id, label) in views {
            let is_focused = focused == Some(view_id);
            if ui.selectable_label(is_focused, &label).clicked() && !is_focused {
                frame_intents.push(GraphIntent::FocusGraphView { view_id });
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
            None
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
        .and_then(|view| view.lens.lens_id.clone())
        .or_else(|| graph_app.default_registry_lens_id().map(str::to_owned))
        .unwrap_or_else(|| LENS_ID_DEFAULT.to_string());
    format!("Lens: {}", lens_id.trim_start_matches("lens:"))
}

fn graph_bar_physics_label(graph_app: &GraphBrowserApp) -> String {
    let physics_id = graph_app
        .default_registry_physics_id()
        .unwrap_or(PHYSICS_PROFILE_LIQUID);
    format!("Physics: {}", physics_id.trim_start_matches("physics:"))
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
        let Some(base_lens) = graph_app
            .workspace
            .graph_runtime
            .views
            .get(&view_id)
            .map(|view| view.lens.clone())
        else {
            ui.label("No active graph view");
            return;
        };

        for (label, lens_id) in [
            ("Default", LENS_ID_DEFAULT),
            ("Semantic Overlay", LENS_ID_SEMANTIC_OVERLAY),
        ] {
            if ui.button(label).clicked() {
                let mut lens = base_lens.clone();
                lens.lens_id = Some(lens_id.to_string());
                frame_intents.push(GraphIntent::SetViewLens { view_id, lens });
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
        for (label, profile_id) in [
            ("Liquid", PHYSICS_PROFILE_LIQUID),
            ("Gas", PHYSICS_PROFILE_GAS),
            ("Solid", PHYSICS_PROFILE_SOLID),
        ] {
            if ui.button(label).clicked() {
                frame_intents.push(GraphIntent::SetPhysicsProfile {
                    profile_id: profile_id.to_string(),
                });
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
    graph_app.workspace.graph_runtime.tag_panel_state = Some(TagPanelState {
        node_key,
        text_input: String::new(),
        icon_picker_open: false,
        pending_icon_override: None,
    });
}

pub(crate) fn render_toolbar_ui(args: Input<'_>) -> Output {
    let Input {
        ctx,
        winit_window,
        state,
        graph_app,
        window,
        tiles_tree,
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
    TopBottomPanel::top("graph_bar")
        .frame(frame)
        .exact_height(TOOLBAR_HEIGHT)
        .show(ctx, |ui| {
            ui.columns(3, |columns| {
                columns[0].horizontal_wrapped(|ui| {
                    render_graph_view_tabs(ui, graph_app, frame_intents);
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

                    let command_button = ui
                        .add(toolbar_button("Cmd"))
                        .on_hover_text("Open command palette (F2)");
                    if command_button.clicked() {
                        graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleCommandPalette);
                    }
                });

                columns[1].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    render_location_search_panel(
                        ui,
                        ctx,
                        state,
                        graph_app,
                        window,
                        tiles_tree,
                        focused_toolbar_node,
                        active_toolbar_pane,
                        local_widget_focus,
                        has_node_panes,
                        is_graph_view,
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
                });
            });
        });

    Output {
        toggle_tile_view_requested,
        open_selected_mode_after_submit,
        toolbar_visible: true,
    }
}
