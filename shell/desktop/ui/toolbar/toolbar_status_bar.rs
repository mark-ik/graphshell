use egui::TopBottomPanel;

use crate::mods::verse;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics::AmbientDiagnosticsAttention;
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use crate::shell::desktop::ui::gui_state::{
    FocusedContentDownloadState, FocusedContentMediaState, FocusedContentStatus,
    ReturnAnchor, RuntimeFocusState, SemanticRegionFocus,
};
use crate::shell::desktop::ui::workbench_host::WorkbenchLayerState;

const STATUS_BAR_HEIGHT: f32 = 24.0;
const STATUS_BAR_URL_MAX_CHARS: usize = 56;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusChipTone {
    Default,
    Weak,
    Notice,
    Warning,
    Success,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StatusChip {
    label: String,
    tone: StatusChipTone,
    tooltip: Option<String>,
}

impl StatusChip {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tone: StatusChipTone::Default,
            tooltip: None,
        }
    }

    fn toned(label: impl Into<String>, tone: StatusChipTone) -> Self {
        Self {
            label: label.into(),
            tone,
            tooltip: None,
        }
    }

    fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ShellStatusBarModel {
    leading: Vec<StatusChip>,
    center: Vec<StatusChip>,
    trailing: Vec<StatusChip>,
}

#[derive(Clone, Debug)]
struct SyncStatusSummary {
    label: String,
    tooltip: String,
    tone: StatusChipTone,
}

fn sync_status_summary() -> SyncStatusSummary {
    let (label, tone, tooltip) = if !verse::is_initialized() {
        (
            "Sync: unavailable".to_string(),
            StatusChipTone::Weak,
            "Sync: Not available".to_string(),
        )
    } else {
        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        if !peers.is_empty() {
            (
                format!(
                    "Sync: connected ({} peer{})",
                    peers.len(),
                    if peers.len() == 1 { "" } else { "s" }
                ),
                StatusChipTone::Success,
                format!(
                    "Sync: Connected ({} peer{})",
                    peers.len(),
                    if peers.len() == 1 { "" } else { "s" }
                ),
            )
        } else {
            (
                "Sync: ready (no peers)".to_string(),
                StatusChipTone::Notice,
                "Sync: Ready (no peers)".to_string(),
            )
        }
    };

    SyncStatusSummary {
        label,
        tooltip,
        tone,
    }
}

fn workbench_layer_state_label(state: WorkbenchLayerState) -> &'static str {
    match state {
        WorkbenchLayerState::GraphOnly => "Graph only",
        WorkbenchLayerState::GraphOverlayActive => "Graph overlay",
        WorkbenchLayerState::WorkbenchOverlayActive => "Workbench overlay",
        WorkbenchLayerState::WorkbenchActive => "Workbench active",
        WorkbenchLayerState::WorkbenchPinned => "Workbench pinned",
        WorkbenchLayerState::WorkbenchOnly => "Workbench only",
    }
}

fn semantic_region_label(region: &SemanticRegionFocus) -> &'static str {
    match region {
        SemanticRegionFocus::ModalDialog => "Modal dialog",
        SemanticRegionFocus::CommandPalette => "Command palette",
        SemanticRegionFocus::ContextPalette => "Context palette",
        SemanticRegionFocus::RadialPalette => "Radial palette",
        SemanticRegionFocus::ClipInspector => "Clip inspector",
        SemanticRegionFocus::HelpPanel => "Help panel",
        SemanticRegionFocus::SceneOverlay => "Scene overlay",
        SemanticRegionFocus::SettingsOverlay => "Settings overlay",
        SemanticRegionFocus::Toolbar => "Command bar",
        SemanticRegionFocus::GraphSurface { .. } => "Graph surface",
        SemanticRegionFocus::NodePane { .. } => "Node pane",
        SemanticRegionFocus::ToolPane { .. } => "Tool pane",
        SemanticRegionFocus::Unspecified => "Unspecified",
    }
}

fn return_anchor_label(anchor: &ReturnAnchor) -> String {
    match anchor {
        ReturnAnchor::ToolSurface(crate::app::ToolSurfaceReturnTarget::Graph(_)) => {
            "Return: graph".to_string()
        }
        ReturnAnchor::ToolSurface(crate::app::ToolSurfaceReturnTarget::Node(_)) => {
            "Return: node".to_string()
        }
        ReturnAnchor::ToolSurface(crate::app::ToolSurfaceReturnTarget::Tool(tool_kind)) => {
            format!("Return: {:?}", tool_kind)
        }
        ReturnAnchor::GraphView(_) => "Return: graph".to_string(),
        ReturnAnchor::Pane(_) => "Return: pane".to_string(),
    }
}

fn load_status_chip(status: servo::LoadStatus) -> Option<StatusChip> {
    match status {
        servo::LoadStatus::Complete => None,
        servo::LoadStatus::Started => {
            Some(StatusChip::toned("Load: loading", StatusChipTone::Notice))
        }
        _ => Some(StatusChip::toned("Load: active", StatusChipTone::Notice)),
    }
}

fn content_media_chip(state: FocusedContentMediaState) -> Option<StatusChip> {
    match state {
        FocusedContentMediaState::Unsupported => None,
        FocusedContentMediaState::Silent => Some(StatusChip::new("Media: silent")),
        FocusedContentMediaState::Playing => Some(StatusChip::new("Media: playing")),
        FocusedContentMediaState::Muted => Some(StatusChip::new("Media: muted")),
    }
}

fn content_download_chip(state: FocusedContentDownloadState) -> Option<StatusChip> {
    match state {
        FocusedContentDownloadState::Unsupported => None,
        FocusedContentDownloadState::Idle => None,
        FocusedContentDownloadState::Active => {
            Some(StatusChip::toned("Downloads: active", StatusChipTone::Notice))
        }
        FocusedContentDownloadState::Recent => {
            Some(StatusChip::toned("Downloads: recent", StatusChipTone::Weak))
        }
    }
}

fn compact_status_bar_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= STATUS_BAR_URL_MAX_CHARS {
        return trimmed.to_string();
    }
    let shortened: String = trimmed
        .chars()
        .take(STATUS_BAR_URL_MAX_CHARS.saturating_sub(1))
        .collect();
    format!("{shortened}…")
}

fn build_shell_status_bar_model(
    workbench_layer_state: WorkbenchLayerState,
    focused_content_status: &FocusedContentStatus,
    runtime_focus_state: Option<&RuntimeFocusState>,
    sync_status: SyncStatusSummary,
    #[cfg(feature = "diagnostics")] diagnostics_attention: Option<&AmbientDiagnosticsAttention>,
) -> ShellStatusBarModel {
    let mut model = ShellStatusBarModel::default();

    model.leading.push(StatusChip::new(format!(
        "Host: {}",
        workbench_layer_state_label(workbench_layer_state)
    )));

    if let Some(focus_state) = runtime_focus_state {
        model.leading.push(StatusChip::new(format!(
            "Focus: {}",
            semantic_region_label(&focus_state.semantic_region)
        )));

        if let Some(top_capture) = focus_state.capture_stack.last() {
            if let Some(return_anchor) = top_capture.return_anchor.as_ref() {
                model.leading.push(StatusChip::new(return_anchor_label(return_anchor)));
            }
            model
                .leading
                .push(StatusChip::new(format!("Capture: {:?}", top_capture.surface)));
        }
    }

    if let Some(url) = focused_content_status.current_url.as_deref() {
        model.center.push(StatusChip::new(compact_status_bar_text(url)));
    }
    if let Some(status_text) = focused_content_status.status_text.as_deref() {
        model.center.push(StatusChip::toned(
            compact_status_bar_text(status_text),
            StatusChipTone::Weak,
        ));
    }
    if model.center.is_empty() {
        model
            .center
            .push(StatusChip::toned("No live content", StatusChipTone::Weak));
    }

    #[cfg(feature = "diagnostics")]
    if let Some(attention) = diagnostics_attention {
        let label = if attention.alert_count == 1 {
            "Diagnostics: 1 alert".to_string()
        } else {
            format!("Diagnostics: {} alerts", attention.alert_count)
        };
        model.trailing.push(
            StatusChip::toned(label, StatusChipTone::Warning).with_tooltip(format!(
                "{}: {}",
                attention.primary_label, attention.primary_summary
            )),
        );
    }

    if matches!(
        workbench_layer_state,
        WorkbenchLayerState::WorkbenchOverlayActive
    ) {
        model.trailing.push(StatusChip::toned(
            "Workbench overlay open",
            StatusChipTone::Notice,
        ));
    }

    if let Some(focus_state) = runtime_focus_state
        && focus_state.overlay_active()
    {
        model
            .trailing
            .push(StatusChip::toned("Overlay active", StatusChipTone::Warning));
    }

    if let Some(load_chip) = load_status_chip(focused_content_status.load_status) {
        model.trailing.push(load_chip);
    }
    if let Some(download_chip) = content_download_chip(focused_content_status.download_state) {
        model.trailing.push(download_chip);
    }
    if let Some(media_chip) = content_media_chip(focused_content_status.media_state) {
        model.trailing.push(media_chip);
    }
    if let Some(zoom_level) = focused_content_status.content_zoom_level {
        model
            .trailing
            .push(StatusChip::new(format!("Zoom: {:.0}%", zoom_level * 100.0)));
    }

    model.trailing.push(
        StatusChip::toned(sync_status.label, sync_status.tone)
            .with_tooltip(sync_status.tooltip),
    );

    model
}

fn chip_color(ctx: &egui::Context, tone: StatusChipTone) -> Option<egui::Color32> {
    let theme_tokens = phase3_resolve_active_theme(None).tokens;
    match tone {
        StatusChipTone::Default => None,
        StatusChipTone::Weak => Some(ctx.style().visuals.weak_text_color()),
        StatusChipTone::Notice => Some(theme_tokens.command_notice),
        StatusChipTone::Warning => Some(ctx.style().visuals.warn_fg_color),
        StatusChipTone::Success => Some(theme_tokens.status_success),
    }
}

fn render_status_chip(ui: &mut egui::Ui, ctx: &egui::Context, chip: &StatusChip) {
    let text = egui::RichText::new(&chip.label).small();
    let text = if let Some(color) = chip_color(ctx, chip.tone) {
        text.color(color)
    } else {
        text
    };
    let response = ui.label(text);
    if let Some(tooltip) = chip.tooltip.as_deref() {
        response.on_hover_text(tooltip);
    }
}

fn render_status_row(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    chips: &[StatusChip],
    reverse: bool,
) {
    if reverse {
        for (index, chip) in chips.iter().rev().enumerate() {
            if index > 0 {
                ui.separator();
            }
            render_status_chip(ui, ctx, chip);
        }
        return;
    }

    for (index, chip) in chips.iter().enumerate() {
        if index > 0 {
            ui.separator();
        }
        render_status_chip(ui, ctx, chip);
    }
}

pub(super) fn render_shell_status_bar(
    ctx: &egui::Context,
    workbench_layer_state: WorkbenchLayerState,
    focused_content_status: &FocusedContentStatus,
    runtime_focus_state: Option<&RuntimeFocusState>,
    #[cfg(feature = "diagnostics")] diagnostics_attention: Option<&AmbientDiagnosticsAttention>,
) -> egui::Rect {
    let model = build_shell_status_bar_model(
        workbench_layer_state,
        focused_content_status,
        runtime_focus_state,
        sync_status_summary(),
        #[cfg(feature = "diagnostics")]
        diagnostics_attention,
    );

    let response = TopBottomPanel::bottom("shell_status_bar")
        .frame(egui::Frame::default().fill(ctx.style().visuals.window_fill).inner_margin(4.0))
        .exact_height(STATUS_BAR_HEIGHT)
        .show(ctx, |ui| {
            ui.columns(3, |columns| {
                columns[0].horizontal_wrapped(|ui| {
                    render_status_row(ui, ctx, &model.leading, false);
                });

                columns[1].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    render_status_row(ui, ctx, &model.center, false);
                });

                columns[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_status_row(ui, ctx, &model.trailing, true);
                });
            });
        });
    response.response.rect
}

#[cfg(test)]
mod tests {
    use super::{
        ShellStatusBarModel, StatusChipTone, SyncStatusSummary, build_shell_status_bar_model,
        render_shell_status_bar,
    };
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::runtime::diagnostics::AmbientDiagnosticsAttention;
    use crate::shell::desktop::ui::gui_state::{
        FocusCaptureEntry, FocusCaptureSurface, FocusedContentDownloadState,
        FocusedContentFeatureSupport, FocusedContentMediaState, FocusedContentStatus,
        ReturnAnchor, RuntimeFocusState, SemanticRegionFocus,
    };
    use crate::shell::desktop::ui::workbench_host::WorkbenchLayerState;
    use crate::shell::desktop::workbench::pane_model::PaneId;

    fn focused_content_status() -> FocusedContentStatus {
        FocusedContentStatus {
            node_key: None,
            renderer_id: None,
            current_url: Some("https://example.com/path".to_string()),
            load_status: servo::LoadStatus::Started,
            status_text: Some("Waiting on renderer".to_string()),
            can_go_back: false,
            can_go_forward: false,
            can_stop_load: false,
            find_in_page: FocusedContentFeatureSupport::Available,
            content_zoom_level: Some(1.25),
            media_state: FocusedContentMediaState::Playing,
            download_state: FocusedContentDownloadState::Active,
        }
    }

    fn runtime_focus_state() -> RuntimeFocusState {
        RuntimeFocusState {
            semantic_region: SemanticRegionFocus::CommandPalette,
            pane_activation: None,
            graph_view_focus: None,
            local_widget_focus: None,
            embedded_content_focus: None,
            capture_stack: vec![FocusCaptureEntry {
                surface: FocusCaptureSurface::CommandPalette,
                return_anchor: Some(ReturnAnchor::Pane(PaneId::new())),
            }],
        }
    }

    fn test_sync_status(label: &str) -> SyncStatusSummary {
        SyncStatusSummary {
            label: label.to_string(),
            tooltip: label.to_string(),
            tone: StatusChipTone::Notice,
        }
    }

    #[cfg(feature = "diagnostics")]
    fn diagnostics_attention(count: usize, summary: &str) -> AmbientDiagnosticsAttention {
        AmbientDiagnosticsAttention {
            alert_count: count,
            primary_label: "Navigator Projection Health".to_string(),
            primary_summary: summary.to_string(),
        }
    }

    fn chip_labels(model: &ShellStatusBarModel) -> (Vec<String>, Vec<String>, Vec<String>) {
        (
            model.leading.iter().map(|chip| chip.label.clone()).collect(),
            model.center.iter().map(|chip| chip.label.clone()).collect(),
            model.trailing.iter().map(|chip| chip.label.clone()).collect(),
        )
    }

    #[test]
    fn status_bar_model_orders_attention_before_ambient_chips() {
        let model = build_shell_status_bar_model(
            WorkbenchLayerState::WorkbenchPinned,
            &focused_content_status(),
            Some(&runtime_focus_state()),
            test_sync_status("Sync: ready"),
            #[cfg(feature = "diagnostics")]
            None,
        );

        let (leading, center, trailing) = chip_labels(&model);
        assert_eq!(
            leading,
            vec![
                "Host: Workbench pinned".to_string(),
                "Focus: Command palette".to_string(),
                "Return: pane".to_string(),
                "Capture: CommandPalette".to_string(),
            ]
        );
        assert_eq!(
            center,
            vec![
                "https://example.com/path".to_string(),
                "Waiting on renderer".to_string(),
            ]
        );
        assert_eq!(
            trailing,
            vec![
                "Overlay active".to_string(),
                "Load: loading".to_string(),
                "Downloads: active".to_string(),
                "Media: playing".to_string(),
                "Zoom: 125%".to_string(),
                "Sync: ready".to_string(),
            ]
        );
    }

    #[test]
    fn status_bar_model_falls_back_when_content_is_unavailable() {
        let model = build_shell_status_bar_model(
            WorkbenchLayerState::GraphOnly,
            &FocusedContentStatus::unavailable(None, None),
            None,
            test_sync_status("Sync: unavailable"),
            #[cfg(feature = "diagnostics")]
            None,
        );

        let (leading, center, trailing) = chip_labels(&model);
        assert_eq!(leading, vec!["Host: Graph only".to_string()]);
        assert_eq!(center, vec!["No live content".to_string()]);
        assert_eq!(trailing, vec!["Sync: unavailable".to_string()]);
    }

    #[test]
    fn status_bar_model_marks_workbench_overlay_open_explicitly() {
        let model = build_shell_status_bar_model(
            WorkbenchLayerState::WorkbenchOverlayActive,
            &FocusedContentStatus::unavailable(None, None),
            None,
            test_sync_status("Sync: ready"),
            #[cfg(feature = "diagnostics")]
            None,
        );

        let (_, _, trailing) = chip_labels(&model);
        assert_eq!(
            trailing,
            vec![
                "Workbench overlay open".to_string(),
                "Sync: ready".to_string(),
            ]
        );
    }

    #[test]
    fn render_shell_status_bar_returns_a_rect() {
        let ctx = egui::Context::default();
        let mut status_bar_rect = None;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            status_bar_rect = Some(render_shell_status_bar(
                ctx,
                WorkbenchLayerState::WorkbenchActive,
                &focused_content_status(),
                Some(&runtime_focus_state()),
                #[cfg(feature = "diagnostics")]
                None,
            ));
        });

        let status_bar_rect = status_bar_rect.expect("status bar should render a rect");
        assert!(status_bar_rect.height() > 0.0);
        assert!(status_bar_rect.width() >= 0.0);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn status_bar_model_promotes_diagnostic_alerts_to_attention_tier() {
        let model = build_shell_status_bar_model(
            WorkbenchLayerState::WorkbenchActive,
            &focused_content_status(),
            Some(&runtime_focus_state()),
            test_sync_status("Sync: ready"),
            Some(&diagnostics_attention(2, "navigation violation receipts observed")),
        );

        let (_, _, trailing) = chip_labels(&model);
        assert_eq!(trailing.first().map(String::as_str), Some("Diagnostics: 2 alerts"));
    }
}