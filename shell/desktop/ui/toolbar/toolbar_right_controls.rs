use crate::app::{GraphBrowserApp, GraphIntent};
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
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
    has_node_panes: bool,
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
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) {
    // Sync status indicator
    render_sync_status_indicator(ui);

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

    let (view_icon, view_tooltip) = if has_node_panes {
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
        has_node_panes,
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
        has_node_panes,
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

/// Render a simple sync status indicator showing Verse P2P status
fn render_sync_status_indicator(ui: &mut egui::Ui) {
    use crate::mods::verse;
    
    // Check if Verse is available
    let (status_char, status_color, tooltip) = if !verse::is_initialized() {
        // Verse not available - show gray dot
        ("○", egui::Color32::from_rgb(128, 128, 128), 
         "Sync: Not available".to_string())
    } else {
        let peers = verse::get_trusted_peers();
        if !peers.is_empty() {
            // Has peers - show green dot
            ("●", egui::Color32::from_rgb(0, 200, 0), 
             format!("Sync: Connected ({} peer{})", peers.len(), if peers.len() == 1 { "" } else { "s" }))
        } else {
            // No peers - show yellow dot
            ("○", egui::Color32::from_rgb(200, 200, 0), 
             "Sync: Ready (no peers)".to_string())
        }
    };

    ui.label(egui::RichText::new(status_char).color(status_color))
        .on_hover_text(tooltip);
}
