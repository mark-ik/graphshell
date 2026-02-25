use crate::app::{
    CommandPaletteShortcut, GraphBrowserApp, GraphIntent, HelpPanelShortcut, LassoMouseBinding,
    OmnibarNonAtOrderPreset, OmnibarPreferredScope, RadialMenuShortcut, ToastAnchorPreference,
};
use crate::shell::desktop::host::running_app_state::{RunningAppState, UserInterfaceCommand};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use egui::Slider;

pub(super) fn render_settings_menu(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    state: &RunningAppState,
    frame_intents: &mut Vec<GraphIntent>,
    location_dirty: &mut bool,
    window: &EmbedderWindow,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) {
    if ui.button("Open Persistence Hub").clicked() {
        graph_app.workspace.show_persistence_panel = true;
        frame_intents.push(GraphIntent::OpenToolPane {
            kind: ToolPaneState::Settings,
        });
        ui.close();
    }
    if ui
        .button(if graph_app.workspace.show_physics_panel {
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
        .button(if graph_app.workspace.show_help_panel {
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
        .button(if graph_app.workspace.show_history_manager {
            "Hide History Manager"
        } else {
            "Show History Manager"
        })
        .clicked()
    {
        let opening = !graph_app.workspace.show_history_manager;
        frame_intents.push(GraphIntent::ToggleHistoryManager);
        if opening {
            frame_intents.push(GraphIntent::OpenToolPane {
                kind: ToolPaneState::HistoryManager,
            });
        }
        ui.close();
    }
    ui.separator();
    ui.label(format!(
        "Toasts: {}",
        super::toast_anchor_label(graph_app.workspace.toast_anchor_preference)
    ));
    for anchor in [
        ToastAnchorPreference::BottomRight,
        ToastAnchorPreference::BottomLeft,
        ToastAnchorPreference::TopRight,
        ToastAnchorPreference::TopLeft,
    ] {
        if ui
            .selectable_label(
                graph_app.workspace.toast_anchor_preference == anchor,
                super::toast_anchor_label(anchor),
            )
            .clicked()
        {
            graph_app.set_toast_anchor_preference(anchor);
        }
    }
    ui.separator();
    ui.label("Graph Zoom");
    let mut zoom_impulse = graph_app.workspace.scroll_zoom_impulse_scale;
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
    let mut zoom_damping = graph_app.workspace.scroll_zoom_inertia_damping;
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
    let mut zoom_min_abs = graph_app.workspace.scroll_zoom_inertia_min_abs;
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
    let mut zoom_requires_ctrl = graph_app.workspace.scroll_zoom_requires_ctrl;
    if ui
        .checkbox(&mut zoom_requires_ctrl, "Scroll Zoom Requires Ctrl")
        .changed()
    {
        graph_app.set_scroll_zoom_requires_ctrl(zoom_requires_ctrl);
    }
    ui.separator();
    ui.label("Input");
    ui.label(format!(
        "Lasso: {}",
        super::lasso_binding_label(graph_app.workspace.lasso_mouse_binding)
    ));
    for binding in [LassoMouseBinding::RightDrag, LassoMouseBinding::ShiftLeftDrag] {
        if ui
            .selectable_label(
                graph_app.workspace.lasso_mouse_binding == binding,
                super::lasso_binding_label(binding),
            )
            .clicked()
        {
            graph_app.set_lasso_mouse_binding(binding);
        }
    }
    ui.label(format!(
        "Command Palette: {}",
        super::command_palette_shortcut_label(graph_app.workspace.command_palette_shortcut)
    ));
    for shortcut in [CommandPaletteShortcut::F2, CommandPaletteShortcut::CtrlK] {
        if ui
            .selectable_label(
                graph_app.workspace.command_palette_shortcut == shortcut,
                super::command_palette_shortcut_label(shortcut),
            )
            .clicked()
        {
            graph_app.set_command_palette_shortcut(shortcut);
        }
    }
    ui.label(format!(
        "Help: {}",
        super::help_shortcut_label(graph_app.workspace.help_panel_shortcut)
    ));
    for shortcut in [HelpPanelShortcut::F1OrQuestion, HelpPanelShortcut::H] {
        if ui
            .selectable_label(
                graph_app.workspace.help_panel_shortcut == shortcut,
                super::help_shortcut_label(shortcut),
            )
            .clicked()
        {
            graph_app.set_help_panel_shortcut(shortcut);
        }
    }
    ui.label(format!(
        "Radial: {}",
        super::radial_shortcut_label(graph_app.workspace.radial_menu_shortcut)
    ));
    for shortcut in [RadialMenuShortcut::F3, RadialMenuShortcut::R] {
        if ui
            .selectable_label(
                graph_app.workspace.radial_menu_shortcut == shortcut,
                super::radial_shortcut_label(shortcut),
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
        super::omnibar_preferred_scope_label(graph_app.workspace.omnibar_preferred_scope)
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
                graph_app.workspace.omnibar_preferred_scope == scope,
                super::omnibar_preferred_scope_label(scope),
            )
            .clicked()
        {
            graph_app.set_omnibar_preferred_scope(scope);
        }
    }
    ui.label(format!(
        "Non-@ Order: {}",
        super::omnibar_non_at_order_label(graph_app.workspace.omnibar_non_at_order)
    ));
    for order in [
        OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal,
    ] {
        if ui
            .selectable_label(
                graph_app.workspace.omnibar_non_at_order == order,
                super::omnibar_non_at_order_label(order),
            )
            .clicked()
        {
            graph_app.set_omnibar_non_at_order(order);
        }
    }
    ui.separator();
    ui.label("Preferences");
    if ui.button("Open Preferences Page").clicked() {
        super::request_open_settings_page(graph_app, frame_intents, "servo:preferences");
        ui.close();
    }
    if ui.button("Open Experimental Preferences").clicked() {
        super::request_open_settings_page(graph_app, frame_intents, "servo:experimental-preferences");
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
