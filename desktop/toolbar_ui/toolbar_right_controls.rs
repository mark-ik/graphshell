use crate::app::{GraphBrowserApp, GraphIntent};
use crate::desktop::tile_kind::TileKind;
use crate::desktop::toolbar_routing::ToolbarOpenMode;
use crate::graph::NodeKey;
use crate::running_app_state::RunningAppState;
use crate::window::EmbedderWindow;
use egui::{WidgetInfo, WidgetType};
use egui_tiles::Tree;
use std::collections::HashSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_toolbar_right_controls(
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
    omnibar_search_session: &mut Option<super::OmnibarSearchSession>,
    frame_intents: &mut Vec<GraphIntent>,
    focused_pane_pin_name: Option<&str>,
    persisted_workspace_names: &HashSet<String>,
    toggle_tile_view_requested: &mut bool,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::desktop::diagnostics::DiagnosticsState,
) {
    ui.menu_button("Settings", |ui| {
        super::render_settings_menu(
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
        .add(super::toolbar_button(view_icon))
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
        .add(super::toolbar_button("Clr"))
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
        .add(super::toolbar_button("Cmd"))
        .on_hover_text("Open command palette (F2)");
    if command_button.clicked() {
        frame_intents.push(GraphIntent::ToggleCommandPalette);
    }

    super::render_workspace_pin_controls(
        ui,
        graph_app,
        has_webview_tiles,
        focused_pane_pin_name,
        persisted_workspace_names,
    );

    super::render_location_search_panel(
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
