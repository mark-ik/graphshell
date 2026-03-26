/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Initial egui_tiles behavior wiring.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use egui::{Color32, Id, Response, Sense, Stroke, TextStyle, Ui, Vec2, WidgetText, vec2};
use egui_tiles::{
    Behavior, Container, SimplificationOptions, TabState, Tile, TileId, Tiles, UiResponse,
};

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphMutation, LifecycleCause, RuntimeEvent, SearchDisplayMode,
    SelectionUpdateMode, ViewAction, WorkbenchIntent,
};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::render;
use crate::render::GraphAction;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::render_backend::{texture_id_from_token, texture_token_from_handle};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE, CHANNEL_UX_CONTRACT_WARNING,
    CHANNEL_VIEWER_FALLBACK_USED, CHANNEL_VIEWER_FALLBACK_WRY_CAPABILITY_MISSING,
    CHANNEL_VIEWER_FALLBACK_WRY_DISABLED_BY_PREFERENCE,
    CHANNEL_VIEWER_FALLBACK_WRY_FEATURE_DISABLED,
};
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::ui::gui_state::RuntimeFocusInspector;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, ViewerId};
use crate::util::{VersoAddress, truncate_with_ellipsis};

use super::selection_range::inclusive_index_range;
use super::tile_kind::TileKind;
use super::tile_runtime;
use super::ux_tree;

#[path = "tile_behavior/node_pane_ui.rs"]
mod node_pane_ui;
#[path = "tile_behavior/pending_intents.rs"]
mod pending_intents;
#[path = "tile_behavior/tab_chrome.rs"]
mod tab_chrome;
#[path = "tile_behavior/tool_pane_ui.rs"]
mod tool_pane_ui;

const PLAINTEXT_HEX_PREVIEW_BYTES: usize = 4096;

enum PlaintextContent {
    Text(String),
    HexPreview(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NodeFrameSplitOfferCandidate {
    frame_name: String,
    hint_count: usize,
}

fn frame_names_for_node(graph_app: &GraphBrowserApp, node_key: NodeKey) -> Vec<String> {
    let mut frame_names = graph_app.sorted_frames_for_node_key(node_key);
    for group in graph_app.arrangement_projection_groups() {
        if group.sub_kind == crate::graph::ArrangementSubKind::FrameMember
            && group.member_keys.contains(&node_key)
            && !frame_names.contains(&group.id)
        {
            frame_names.push(group.id);
        }
    }
    frame_names
}

fn frame_key_for_name(graph_app: &GraphBrowserApp, frame_name: &str) -> Option<NodeKey> {
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    graph_app
        .domain_graph()
        .get_node_by_url(&frame_url)
        .map(|(frame_key, _)| frame_key)
}

fn frame_split_offer_suppressed_for_name(graph_app: &GraphBrowserApp, frame_name: &str) -> bool {
    frame_key_for_name(graph_app, frame_name)
        .and_then(|frame_key| {
            graph_app
                .domain_graph()
                .frame_split_offer_suppressed(frame_key)
        })
        .unwrap_or(false)
}

fn primary_frame_name_for_node(graph_app: &GraphBrowserApp, node_key: NodeKey) -> Option<String> {
    let frame_names = frame_names_for_node(graph_app, node_key);
    if let Some(current_frame_name) = graph_app.current_frame_name()
        && frame_names.iter().any(|name| name == current_frame_name)
    {
        return Some(current_frame_name.to_string());
    }
    frame_names.into_iter().next()
}

fn node_frame_split_offer_candidate(
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) -> Option<NodeFrameSplitOfferCandidate> {
    for frame_name in frame_names_for_node(graph_app, node_key) {
        if graph_app.current_frame_name() == Some(frame_name.as_str())
            || graph_app.is_frame_split_offer_dismissed_for_session(&frame_name)
        {
            continue;
        }

        let Some(frame_key) = frame_key_for_name(graph_app, &frame_name) else {
            continue;
        };
        if graph_app
            .domain_graph()
            .frame_split_offer_suppressed(frame_key)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(hints) = graph_app.domain_graph().frame_layout_hints(frame_key) else {
            continue;
        };
        if hints.is_empty() {
            continue;
        }

        return Some(NodeFrameSplitOfferCandidate {
            frame_name,
            hint_count: hints.len(),
        });
    }
    None
}

fn decode_plaintext_content(bytes: &[u8]) -> PlaintextContent {
    match std::str::from_utf8(bytes) {
        Ok(text) => PlaintextContent::Text(text.to_string()),
        Err(_) => {
            let preview_len = bytes.len().min(PLAINTEXT_HEX_PREVIEW_BYTES);
            let mut hex = String::new();
            for (row, chunk) in bytes[..preview_len].chunks(16).enumerate() {
                let offset = row * 16;
                hex.push_str(&format!("{offset:08x}: "));
                for byte in chunk {
                    hex.push_str(&format!("{byte:02x} "));
                }
                hex.push('\n');
            }
            if bytes.len() > preview_len {
                hex.push_str("\n... truncated binary preview ...\n");
            }
            PlaintextContent::HexPreview(hex)
        }
    }
}

fn file_path_from_node_url(url: &str) -> Result<PathBuf, String> {
    let parsed = url::Url::parse(url).map_err(|err| format!("Invalid URL: {err}"))?;
    if parsed.scheme() != "file" {
        return Err("Embedded plaintext viewer currently supports file:// URLs only.".to_string());
    }

    parsed
        .to_file_path()
        .map_err(|_| "Could not convert file:// URL to local path.".to_string())
}

fn ensure_local_file_access_allowed(path: &PathBuf) -> Result<(), String> {
    let Some(home_dir) = dirs::home_dir() else {
        return Err("Home directory unavailable; local file access is blocked.".to_string());
    };

    let canonical_home = home_dir.canonicalize().map_err(|err| {
        format!(
            "Failed to resolve home directory '{}': {err}",
            home_dir.display()
        )
    })?;
    let canonical_path = path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve '{}': {err}", path.display()))?;

    if canonical_path.starts_with(&canonical_home) {
        Ok(())
    } else {
        Err(format!(
            "Access denied for '{}'. Embedded file viewers currently allow only paths inside '{}'.",
            canonical_path.display(),
            canonical_home.display()
        ))
    }
}

fn guarded_file_path_from_node_url(url: &str) -> Result<PathBuf, String> {
    let path = file_path_from_node_url(url)?;
    ensure_local_file_access_allowed(&path)?;
    Ok(path)
}

fn load_plaintext_content_for_node(url: &str) -> Result<PlaintextContent, String> {
    let path = guarded_file_path_from_node_url(url)?;
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("Failed to read '{}': {err}", path.display()))?;
    Ok(decode_plaintext_content(&bytes))
}

#[derive(Clone, Copy)]
enum WryUnavailableReason {
    FeatureDisabled,
    CapabilityMissing,
    DisabledByPreference,
}

impl WryUnavailableReason {
    fn diagnostics_channel(self) -> &'static str {
        match self {
            WryUnavailableReason::FeatureDisabled => CHANNEL_VIEWER_FALLBACK_WRY_FEATURE_DISABLED,
            WryUnavailableReason::CapabilityMissing => {
                CHANNEL_VIEWER_FALLBACK_WRY_CAPABILITY_MISSING
            }
            WryUnavailableReason::DisabledByPreference => {
                CHANNEL_VIEWER_FALLBACK_WRY_DISABLED_BY_PREFERENCE
            }
        }
    }

    fn message(self) -> &'static str {
        match self {
            WryUnavailableReason::FeatureDisabled => "Wry backend is not compiled in this build.",
            WryUnavailableReason::CapabilityMissing => {
                "Runtime capability 'viewer:wry' is unavailable."
            }
            WryUnavailableReason::DisabledByPreference => {
                "Wry backend is disabled. Enable it in Settings -> Viewer Backends."
            }
        }
    }
}

fn wry_unavailable_reason(graph_app: &GraphBrowserApp) -> Option<WryUnavailableReason> {
    if !cfg!(feature = "wry") {
        return Some(WryUnavailableReason::FeatureDisabled);
    }
    if !crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry") {
        return Some(WryUnavailableReason::CapabilityMissing);
    }
    if !graph_app.wry_enabled() {
        return Some(WryUnavailableReason::DisabledByPreference);
    }
    None
}

fn request_viewer_backend_swap(
    graph_app: &mut GraphBrowserApp,
    state: &NodePaneState,
    viewer_id_override: Option<ViewerId>,
) {
    graph_app.enqueue_workbench_intent(WorkbenchIntent::SwapViewerBackend {
        pane: state.pane_id,
        node: state.node,
        viewer_id_override,
    });
}

fn render_node_viewer_backend_selector(
    ui: &mut Ui,
    graph_app: &mut GraphBrowserApp,
    state: &mut NodePaneState,
) {
    ui.horizontal_wrapped(|ui| {
        ui.small("Render With:");

        let auto_selected = state.viewer_id_override.is_none();
        if ui.selectable_label(auto_selected, "Auto").clicked() {
            request_viewer_backend_swap(graph_app, state, None);
        }

        let webview_selected = state
            .viewer_id_override
            .as_ref()
            .is_some_and(|viewer| viewer.as_str() == "viewer:webview");
        if ui.selectable_label(webview_selected, "WebView").clicked() {
            request_viewer_backend_swap(graph_app, state, Some(ViewerId::new("viewer:webview")));
        }

        let wry_selected = state
            .viewer_id_override
            .as_ref()
            .is_some_and(|viewer| viewer.as_str() == "viewer:wry");
        let wry_disabled_reason = wry_unavailable_reason(graph_app);
        let wry_response = ui.add_enabled(
            wry_disabled_reason.is_none(),
            egui::Button::new("Wry").selected(wry_selected),
        );
        if wry_response.clicked() {
            request_viewer_backend_swap(graph_app, state, Some(ViewerId::new("viewer:wry")));
        }
        if let Some(reason) = wry_disabled_reason {
            wry_response.on_hover_text(reason.message());
        }
    });
}

fn render_markdown_embedded(ui: &mut Ui, markdown: &str) {
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            ui.label(egui::RichText::new(rest).strong());
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            ui.label(egui::RichText::new(rest).strong().size(18.0));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            ui.label(egui::RichText::new(rest).strong().size(22.0));
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            ui.horizontal(|ui| {
                ui.label("•");
                ui.label(rest);
            });
        } else {
            ui.label(line);
        }
    }
}

#[cfg(test)]
mod file_access_guard_tests {
    use super::{ensure_local_file_access_allowed, guarded_file_path_from_node_url};

    #[test]
    fn file_access_guard_allows_paths_inside_home_directory() {
        let home = dirs::home_dir().expect("home directory should exist for this test");
        assert!(ensure_local_file_access_allowed(&home).is_ok());
    }

    #[test]
    fn file_access_guard_rejects_missing_file_urls_before_read() {
        let home = dirs::home_dir().expect("home directory should exist for this test");
        let url = url::Url::from_file_path(home.join("graphshell_missing_ucc_guard_test.txt"))
            .expect("file URL should build");

        let result = guarded_file_path_from_node_url(url.as_str());
        assert!(result.is_err());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingOpenMode {
    SplitHorizontal,
    QuarterPane,
    HalfPane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingOpenNode {
    pub key: NodeKey,
    pub mode: PendingOpenMode,
}

pub(crate) enum TilePendingIntent {
    ViewAction(ViewAction),
    GraphMutation(GraphMutation),
    RuntimeEvent(RuntimeEvent),
    GraphIntent(GraphIntent),
}

impl From<ViewAction> for TilePendingIntent {
    fn from(value: ViewAction) -> Self {
        Self::ViewAction(value)
    }
}

impl From<GraphMutation> for TilePendingIntent {
    fn from(value: GraphMutation) -> Self {
        Self::GraphMutation(value)
    }
}

impl From<RuntimeEvent> for TilePendingIntent {
    fn from(value: RuntimeEvent) -> Self {
        Self::RuntimeEvent(value)
    }
}

impl From<GraphIntent> for TilePendingIntent {
    fn from(value: GraphIntent) -> Self {
        if let Some(action) = value.as_view_action() {
            return Self::ViewAction(action);
        }
        if let Some(mutation) = value.as_graph_mutation() {
            return Self::GraphMutation(mutation);
        }
        if let Some(event) = value.as_runtime_event() {
            return Self::RuntimeEvent(event);
        }
        Self::GraphIntent(value)
    }
}

impl From<TilePendingIntent> for GraphIntent {
    fn from(value: TilePendingIntent) -> Self {
        match value {
            TilePendingIntent::ViewAction(action) => action.into(),
            TilePendingIntent::GraphMutation(mutation) => mutation.into(),
            TilePendingIntent::RuntimeEvent(event) => event.into(),
            TilePendingIntent::GraphIntent(intent) => intent,
        }
    }
}

pub(crate) struct GraphshellTileBehavior<'a> {
    pub graph_app: &'a mut GraphBrowserApp,
    pub control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    search_matches: &'a HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_filter_mode: bool,
    search_query_active: bool,
    pending_open_nodes: Vec<PendingOpenNode>,
    pending_closed_nodes: Vec<NodeKey>,
    pending_post_render_intents: Vec<TilePendingIntent>,
    pending_tab_drag_stopped_nodes: HashSet<NodeKey>,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    #[cfg(feature = "diagnostics")]
    runtime_focus_inspector: Option<RuntimeFocusInspector>,
}

impl<'a> GraphshellTileBehavior<'a> {
    pub fn new(
        graph_app: &'a mut GraphBrowserApp,
        control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
        tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        search_matches: &'a HashSet<NodeKey>,
        active_search_match: Option<NodeKey>,
        search_filter_mode: bool,
        search_query_active: bool,
        #[cfg(feature = "diagnostics")]
        diagnostics_state: &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
        #[cfg(feature = "diagnostics")] runtime_focus_inspector: Option<RuntimeFocusInspector>,
    ) -> Self {
        Self {
            graph_app,
            control_panel,
            tile_favicon_textures,
            search_matches,
            active_search_match,
            search_filter_mode,
            search_query_active,
            pending_open_nodes: Vec::new(),
            pending_closed_nodes: Vec::new(),
            pending_post_render_intents: Vec::new(),
            pending_tab_drag_stopped_nodes: HashSet::new(),
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            #[cfg(feature = "diagnostics")]
            runtime_focus_inspector,
        }
    }

    pub fn take_pending_open_nodes(&mut self) -> Vec<PendingOpenNode> {
        std::mem::take(&mut self.pending_open_nodes)
    }

    pub fn take_pending_closed_nodes(&mut self) -> Vec<NodeKey> {
        std::mem::take(&mut self.pending_closed_nodes)
    }

    pub fn take_pending_post_render_intents(&mut self) -> Vec<TilePendingIntent> {
        std::mem::take(&mut self.pending_post_render_intents)
    }

    pub fn take_pending_tab_drag_stopped_nodes(&mut self) -> HashSet<NodeKey> {
        std::mem::take(&mut self.pending_tab_drag_stopped_nodes)
    }

    fn queue_post_render_intent<T>(&mut self, intent: T)
    where
        T: Into<TilePendingIntent>,
    {
        pending_intents::queue_post_render_intent(self, intent);
    }

    fn extend_post_render_intents<I, T>(&mut self, intents: I)
    where
        I: IntoIterator<Item = T>,
        T: Into<TilePendingIntent>,
    {
        pending_intents::extend_post_render_intents(self, intents);
    }

    fn hash_favicon(width: u32, height: u32, rgba: &[u8]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        width.hash(&mut hasher);
        height.hash(&mut hasher);
        rgba.hash(&mut hasher);
        hasher.finish()
    }

    #[cfg(feature = "diagnostics")]
    fn render_tool_pane_placeholder(
        ui: &mut Ui,
        kind: &crate::shell::desktop::workbench::pane_model::ToolPaneState,
    ) {
        let title = kind.title();
        ui.heading(title);
        ui.separator();
        ui.label(format!(
            "{title} tool pane is not yet rendered in the workbench."
        ));
    }

    #[cfg(feature = "diagnostics")]
    fn accessibility_inspector_snapshot(
        graph_app: &GraphBrowserApp,
    ) -> AccessibilityInspectorSnapshot {
        let focused_selection = graph_app.focused_selection();
        let selected_node_count = focused_selection.len();
        let total_nodes = graph_app.domain_graph().node_count();
        let selected_node = focused_selection.primary().and_then(|node_key| {
            let node = graph_app.domain_graph().get_node(node_key)?;
            let selection =
                crate::shell::desktop::runtime::registries::phase0_select_viewer_for_content(
                    &node.url,
                    node.mime_hint.as_deref(),
                );
            let capabilities = selection.capabilities.clone();

            Some(AccessibilityInspectorSelectedNodeSnapshot {
                node_key,
                node_url: node.url.clone(),
                viewer_id: selection.viewer_id,
                accessibility_level: format!("{:?}", capabilities.accessibility.level),
                accessibility_reason: capabilities.accessibility.reason.clone(),
                runtime_webview_mapped: graph_app.get_webview_for_node(node_key).is_some(),
                runtime_blocked: graph_app.runtime_block_state_for_node(node_key).is_some(),
                runtime_crashed: graph_app.runtime_crash_state_for_node(node_key).is_some(),
                affordance_projection:
                    crate::shell::desktop::ui::gui::Gui::selected_node_affordance_projection(
                        node_key,
                    ),
            })
        });

        AccessibilityInspectorSnapshot {
            total_nodes,
            selected_node_count,
            selected_node,
        }
    }

    #[cfg(feature = "diagnostics")]
    fn accessibility_bridge_health_snapshot(
        _graph_app: &GraphBrowserApp,
    ) -> AccessibilityBridgeHealthSnapshot {
        // Placeholder snapshot capturing bridge health state structure.
        // In production, these would be populated by querying the bridge subsystem,
        // diagnostics channels, and runtime health state.
        // For now, we surface the structure so bridge health observability is visible.

        let update_queue_size = 0; // Would query bridge pending updates queue
        let anchor_count = 0; // Would count active WebView → egui::Id anchors
        let dropped_update_count = 0; // Would query cumulative dropped update counter
        let focus_target = None; // Would query current focus target in tree
        let degradation_state = "none".to_string(); // Would query bridge health status

        AccessibilityBridgeHealthSnapshot {
            update_queue_size,
            anchor_count,
            dropped_update_count,
            focus_target,
            degradation_state,
        }
    }

    #[cfg(feature = "diagnostics")]
    fn graph_reader_snapshot(graph_app: &GraphBrowserApp) -> GraphReaderSnapshot {
        let node_count = graph_app.domain_graph().node_count();
        let mode = match graph_app.graph_reader_mode() {
            Some(crate::app::GraphReaderModeState::Room { .. }) => GraphReaderMode::Room,
            Some(crate::app::GraphReaderModeState::Map { .. }) if node_count > 0 => {
                GraphReaderMode::Map
            }
            _ => GraphReaderMode::Off,
        };

        GraphReaderSnapshot {
            mode,
            entry_point_reachable: true,
            degraded_reason: Some(if node_count == 0 {
                "Starter canonical projection path is active, but no graph content is available yet for Map/Room output."
                        .to_string()
            } else {
                "Starter canonical projection is active: deterministic Map output, focused Room grouping, and starter Graph Reader Room/Map action routing are exposed through the UxTree -> AccessKit path; broader navigation/action coverage is still incomplete."
                        .to_string()
            }),
        }
    }

    #[cfg(feature = "diagnostics")]
    fn render_accessibility_inspector_scaffold(ui: &mut Ui, graph_app: &GraphBrowserApp) {
        let snapshot = Self::accessibility_inspector_snapshot(graph_app);
        let bridge_health = Self::accessibility_bridge_health_snapshot(graph_app);
        let graph_reader = Self::graph_reader_snapshot(graph_app);
        let uxtree_snapshot = ux_tree::latest_snapshot();
        let uxtree_roots = uxtree_snapshot
            .as_ref()
            .map(|snapshot| snapshot.semantic_nodes.len())
            .unwrap_or(0);
        let uxtree_violation = uxtree_snapshot
            .as_ref()
            .and_then(ux_tree::presentation_id_consistency_violation);

        if let Some(message) = uxtree_violation.as_deref() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_CONTRACT_WARNING,
                byte_len: message.len(),
            });
        }

        ui.heading("Accessibility Inspector");
        ui.separator();
        ui.small(
            "Functional scaffold for bridge/tree diagnostics and future accessibility controls.",
        );

        egui::Grid::new("accessibility_inspector_summary")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Graph nodes");
                ui.monospace(snapshot.total_nodes.to_string());
                ui.end_row();

                ui.strong("Selected nodes");
                ui.monospace(snapshot.selected_node_count.to_string());
                ui.end_row();

                ui.strong("UxTree semantic nodes");
                ui.monospace(uxtree_roots.to_string());
                ui.end_row();

                ui.strong("UxTree probe");
                ui.monospace(if uxtree_violation.is_some() {
                    "violation"
                } else {
                    "ok"
                });
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.strong("Selected node accessibility profile");
        match snapshot.selected_node {
            Some(selected) => {
                egui::Grid::new("accessibility_selected_node_profile")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Node");
                        ui.monospace(format!("{:?}", selected.node_key));
                        ui.end_row();

                        ui.label("URL");
                        ui.label(selected.node_url);
                        ui.end_row();

                        ui.label("Viewer");
                        ui.monospace(selected.viewer_id);
                        ui.end_row();

                        ui.label("Accessibility conformance");
                        ui.monospace(selected.accessibility_level);
                        ui.end_row();

                        ui.label("Conformance reason");
                        ui.label(
                            selected
                                .accessibility_reason
                                .unwrap_or_else(|| "none".to_string()),
                        );
                        ui.end_row();

                        ui.label("Runtime webview mapped");
                        ui.monospace(selected.runtime_webview_mapped.to_string());
                        ui.end_row();

                        ui.label("Runtime blocked");
                        ui.monospace(selected.runtime_blocked.to_string());
                        ui.end_row();

                        ui.label("Runtime crashed");
                        ui.monospace(selected.runtime_crashed.to_string());
                        ui.end_row();

                        let affordance_status = selected
                            .affordance_projection
                            .as_ref()
                            .map(|projection| {
                                if projection.status_tokens.is_empty() {
                                    "none".to_string()
                                } else {
                                    projection.status_tokens.join(", ")
                                }
                            })
                            .unwrap_or_else(|| "none".to_string());
                        ui.label("Compositor affordance status");
                        ui.label(affordance_status);
                        ui.end_row();

                        let lifecycle_status = selected
                            .affordance_projection
                            .as_ref()
                            .map(|projection| projection.lifecycle_label)
                            .unwrap_or("none");
                        ui.label("Lifecycle treatment");
                        ui.monospace(lifecycle_status);
                        ui.end_row();

                        let aria_busy = selected
                            .affordance_projection
                            .as_ref()
                            .is_some_and(|projection| projection.aria_busy);
                        ui.label("Projected aria-busy");
                        ui.monospace(aria_busy.to_string());
                        ui.end_row();

                        let glyphs = selected
                            .affordance_projection
                            .as_ref()
                            .map(|projection| {
                                if projection.glyph_descriptions.is_empty() {
                                    "none".to_string()
                                } else {
                                    projection.glyph_descriptions.join(", ")
                                }
                            })
                            .unwrap_or_else(|| "none".to_string());
                        ui.label("Rendered lens glyphs");
                        ui.label(glyphs);
                        ui.end_row();
                    });
            }
            None => {
                ui.small("No selected node. Select a node to inspect viewer accessibility profile and runtime bridge state.");
            }
        }

        // Bridge health diagnostics section
        ui.add_space(8.0);
        ui.separator();
        ui.strong("WebView bridge health diagnostics");
        ui.small("Accessibility tree injection state and degradation indicators.");

        egui::Grid::new("accessibility_bridge_health")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Update queue size");
                ui.monospace(bridge_health.update_queue_size.to_string());
                ui.end_row();

                ui.label("Active anchors");
                ui.monospace(bridge_health.anchor_count.to_string());
                ui.end_row();

                ui.label("Dropped updates");
                ui.monospace(bridge_health.dropped_update_count.to_string());
                ui.end_row();

                ui.label("Focus target");
                ui.label(bridge_health.focus_target.as_deref().unwrap_or("none"));
                ui.end_row();

                ui.label("Bridge health");
                let color = match bridge_health.degradation_state.as_str() {
                    "none" => egui::Color32::from_rgb(100, 200, 130),
                    "warning" => egui::Color32::from_rgb(220, 180, 60),
                    "error" => egui::Color32::from_rgb(220, 100, 100),
                    _ => egui::Color32::GRAY,
                };
                ui.colored_label(color, &bridge_health.degradation_state);
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.separator();
        ui.strong("Graph Reader (starter canonical projection)");
        ui.small(
            "Entry point is reachable and now reports starter Map/Room projection status while full focus and action routing land.",
        );

        egui::Grid::new("accessibility_graph_reader_scaffold")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Mode");
                ui.monospace(graph_reader.mode.label());
                ui.end_row();

                ui.label("Entry point reachable");
                ui.monospace(graph_reader.entry_point_reachable.to_string());
                ui.end_row();

                ui.label("Status");
                ui.colored_label(egui::Color32::from_rgb(220, 180, 60), "degraded");
                ui.end_row();

                ui.label("Degradation reason");
                ui.label(graph_reader.degraded_reason.as_deref().unwrap_or("none"));
                ui.end_row();
            });

        ui.add_enabled(false, egui::Button::new("Enter Room Mode (scaffold)"));
        ui.add_enabled(false, egui::Button::new("Enter Map Mode (scaffold)"));
    }

    fn favicon_texture_id(&mut self, ui: &Ui, node_key: NodeKey) -> Option<egui::TextureId> {
        let (favicon_rgba, favicon_width, favicon_height) = {
            let node = self.graph_app.domain_graph().get_node(node_key)?;
            (
                node.favicon_rgba.clone()?,
                node.favicon_width as usize,
                node.favicon_height as usize,
            )
        };
        if favicon_width == 0 || favicon_height == 0 {
            self.tile_favicon_textures.remove(&node_key);
            return None;
        }
        let expected_len = favicon_width * favicon_height * 4;
        if favicon_rgba.len() != expected_len {
            self.tile_favicon_textures.remove(&node_key);
            return None;
        }

        let favicon_hash =
            Self::hash_favicon(favicon_width as u32, favicon_height as u32, &favicon_rgba);

        let handle = if let Some((cached_hash, handle)) = self.tile_favicon_textures.get(&node_key)
        {
            if *cached_hash == favicon_hash {
                handle.clone()
            } else {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [favicon_width, favicon_height],
                    &favicon_rgba,
                );
                let handle = ui.ctx().load_texture(
                    format!("tile-favicon-{node_key:?}-{favicon_hash}"),
                    image,
                    Default::default(),
                );
                self.tile_favicon_textures
                    .insert(node_key, (favicon_hash, handle.clone()));
                handle
            }
        } else {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [favicon_width, favicon_height],
                &favicon_rgba,
            );
            let handle = ui.ctx().load_texture(
                format!("tile-favicon-{node_key:?}-{favicon_hash}"),
                image,
                Default::default(),
            );
            self.tile_favicon_textures
                .insert(node_key, (favicon_hash, handle.clone()));
            handle
        };

        let texture_token = texture_token_from_handle(&handle);
        Some(texture_id_from_token(texture_token))
    }

    fn should_detach_tab_on_drag_stop(
        ui: &Ui,
        tab_rect: egui::Rect,
        detach_band_margin: f32,
    ) -> bool {
        // Treat release clearly outside the tab strip band as "detach tab to split".
        // Horizontal motion within the tab strip should keep normal tab reorder/group behavior.
        let Some(pointer) = ui.ctx().pointer_interact_pos() else {
            return false;
        };
        pointer.y < tab_rect.top() - detach_band_margin
            || pointer.y > tab_rect.bottom() + detach_band_margin
    }

    fn tab_group_node_order_for_tile(
        tiles: &Tiles<TileKind>,
        tile_id: TileId,
    ) -> Option<Vec<NodeKey>> {
        for (_, tile) in tiles.iter() {
            let Tile::Container(Container::Tabs(tabs)) = tile else {
                continue;
            };
            if !tabs.children.contains(&tile_id) {
                continue;
            }
            let mut out = Vec::new();
            for child_id in &tabs.children {
                if let Some(Tile::Pane(TileKind::Node(state))) = tiles.get(*child_id) {
                    out.push(state.node);
                }
            }
            return Some(out);
        }
        None
    }

    fn activate_successor_tab_in_parent_before_close(tiles: &mut Tiles<TileKind>, tile_id: TileId) {
        let Some(parent_id) = tiles.parent_of(tile_id) else {
            return;
        };
        let Some(Tile::Container(Container::Tabs(tabs))) = tiles.get_mut(parent_id) else {
            return;
        };
        if tabs.active != Some(tile_id) {
            return;
        }

        let Some(index) = tabs.children.iter().position(|child| *child == tile_id) else {
            return;
        };
        let successor = tabs.children.get(index + 1).copied().or_else(|| {
            index
                .checked_sub(1)
                .and_then(|left| tabs.children.get(left).copied())
        });
        if let Some(next_active) = successor {
            tabs.set_active(next_active);
        }
    }
}

impl<'a> Behavior<TileKind> for GraphshellTileBehavior<'a> {
    fn simplification_options(&self) -> SimplificationOptions {
        let workbench_surface = registries::phase3_resolve_active_workbench_surface_profile();

        SimplificationOptions {
            all_panes_must_have_tabs: workbench_surface.profile.layout.all_panes_must_have_tabs,
            ..SimplificationOptions::default()
        }
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut TileKind) -> UiResponse {
        match pane {
            TileKind::Pane(view) => match view {
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref) => {
                    self.render_graph_pane(ui, view_ref.graph_view_id);
                }
                crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state) => {
                    self.render_node_pane(ui, state);
                }
                #[cfg(feature = "diagnostics")]
                crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool) => {
                    self.render_tool_pane(ui, tool);
                }
            },
            TileKind::Graph(view_ref) => {
                self.render_graph_pane(ui, view_ref.graph_view_id);
            }
            TileKind::Node(state) => {
                self.render_node_pane(ui, state);
            }
            #[cfg(feature = "diagnostics")]
            TileKind::Tool(tool) => {
                self.render_tool_pane(ui, tool);
            }
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &TileKind) -> WidgetText {
        self.tab_title_for_tile(pane)
    }

    fn tab_ui(
        &mut self,
        tiles: &mut Tiles<TileKind>,
        ui: &mut Ui,
        id: Id,
        tile_id: TileId,
        state: &TabState,
    ) -> Response {
        self.render_tab_ui(tiles, ui, id, tile_id, state)
    }

    fn is_tab_closable(&self, tiles: &Tiles<TileKind>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(_),
            ))) => false,
            Some(Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Node(_),
            ))) => true,
            #[cfg(feature = "diagnostics")]
            Some(Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(_),
            ))) => true,
            Some(Tile::Pane(TileKind::Node(_))) => true,
            Some(Tile::Pane(TileKind::Graph(_))) => false,
            #[cfg(feature = "diagnostics")]
            Some(Tile::Pane(TileKind::Tool(_))) => true,
            _ => false,
        }
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<TileKind>, tile_id: TileId) -> bool {
        Self::activate_successor_tab_in_parent_before_close(tiles, tile_id);

        if let Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        ))) = tiles.get(tile_id)
        {
            let node_key = state.node;
            self.graph_app.demote_node_to_cold_with_cause(
                node_key,
                crate::app::LifecycleCause::ExplicitClose,
            );
            self.pending_closed_nodes.push(node_key);
            self.graph_app
                .workspace
                .graph_runtime
                .selected_tab_nodes
                .remove(&node_key);
            if self.graph_app.workspace.graph_runtime.tab_selection_anchor == Some(node_key) {
                self.graph_app.workspace.graph_runtime.tab_selection_anchor = None;
            }
        }
        if let Some(Tile::Pane(TileKind::Node(state))) = tiles.get(tile_id) {
            let node_key = state.node;
            // DismissTile semantics: demote to Cold so the node keeps its edges
            // but loses its live tile. The webview runtime teardown still happens
            // via pending_closed_nodes → release_node_runtime_for_pane, which
            // respects Cold lifecycle and does not re-promote to Warm.
            self.graph_app.demote_node_to_cold_with_cause(
                node_key,
                crate::app::LifecycleCause::ExplicitClose,
            );
            self.pending_closed_nodes.push(node_key);
            self.graph_app
                .workspace
                .graph_runtime
                .selected_tab_nodes
                .remove(&node_key);
            if self.graph_app.workspace.graph_runtime.tab_selection_anchor == Some(node_key) {
                self.graph_app.workspace.graph_runtime.tab_selection_anchor = None;
            }
        }
        #[cfg(feature = "diagnostics")]
        if let Some(Tile::Pane(TileKind::Tool(_))) = tiles.get(tile_id) {
            // No extra cleanup needed for tool pane
        }
        true
    }
}

/// Render a per-pane overlay showing lens controls and graph-pane actions.
///
/// The overlay appears as a translucent bar in the top-right corner of the graph pane.
/// Intended to satisfy per-pane lens selection without exposing shared-layout toggles.
fn render_graph_pane_overlay(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    pane_rect: egui::Rect,
    pending_intents: &mut Vec<TilePendingIntent>,
) {
    let Some(view) = app.workspace.graph_runtime.views.get(&view_id) else {
        return;
    };
    let lens_name = view
        .lens
        .lens_id
        .clone()
        .unwrap_or_else(|| view.lens.name.clone());
    let current_lens_id = view
        .lens
        .lens_id
        .clone()
        .unwrap_or_else(|| crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string());
    let base_lens = view.lens.clone();

    // Overlay anchored to top-right of the pane, with a small margin.
    let overlay_width = 150.0;
    let overlay_pos = egui::pos2(pane_rect.max.x - overlay_width - 4.0, pane_rect.min.y + 4.0);

    egui::Area::new(egui::Id::new("graph_pane_overlay").with(view_id))
        .fixed_pos(overlay_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 24, 30, 180))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.set_width(overlay_width - 8.0);

                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Split Graph")
                                        .small()
                                        .color(egui::Color32::from_rgb(190, 210, 230)),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Create a split graph pane with a new graph view")
                            .clicked()
                        {
                            app.enqueue_workbench_intent(WorkbenchIntent::SplitPane {
                                source_pane: crate::shell::desktop::workbench::pane_model::PaneId::new(),
                                direction: crate::shell::desktop::workbench::pane_model::SplitDirection::Horizontal,
                            });
                        }
                    });

                    // Lens row: display current lens with a click-to-reset affordance.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Lens:")
                                .small()
                                .color(egui::Color32::from_rgb(160, 175, 190)),
                        );
                        let display = crate::util::truncate_with_ellipsis(
                            lens_name.trim_start_matches("lens:"),
                            12,
                        );
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(display)
                                        .small()
                                        .color(egui::Color32::from_rgb(210, 225, 240)),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Click to reset lens to default")
                            .clicked()
                        {
                            pending_intents.push(GraphIntent::SetViewLens {
                                view_id,
                                lens: crate::app::LensConfig::default(),
                            }
                            .into());
                        }
                    });

                    let lens_input_id = egui::Id::new("graph_pane_lens_input").with(view_id);
                    let mut lens_input = ctx
                        .data_mut(|d| d.get_persisted::<String>(lens_input_id))
                        .unwrap_or_else(|| current_lens_id.clone());

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Lens ID")
                                .small()
                                .color(egui::Color32::from_rgb(160, 175, 190)),
                        );
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut lens_input)
                                .desired_width(88.0)
                                .hint_text("lens:..."),
                        );
                        let submit_with_enter =
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.small_button("Apply").clicked() || submit_with_enter {
                            let requested = lens_input.trim();
                            if !requested.is_empty() {
                                let mut lens = base_lens.clone();
                                lens.lens_id = Some(requested.to_string());
                                pending_intents.push(GraphIntent::SetViewLens { view_id, lens }.into());
                            }
                        }
                    });

                    ctx.data_mut(|d| d.insert_persisted(lens_input_id, lens_input));

                    ui.small("Layout: local-per-view");
                });
        });
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone)]
struct AccessibilityInspectorSelectedNodeSnapshot {
    node_key: NodeKey,
    node_url: String,
    viewer_id: &'static str,
    accessibility_level: String,
    accessibility_reason: Option<String>,
    runtime_webview_mapped: bool,
    runtime_blocked: bool,
    runtime_crashed: bool,
    affordance_projection:
        Option<crate::shell::desktop::ui::gui::TileAffordanceAccessibilityProjection>,
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone)]
struct AccessibilityInspectorSnapshot {
    total_nodes: usize,
    selected_node_count: usize,
    selected_node: Option<AccessibilityInspectorSelectedNodeSnapshot>,
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone)]
struct AccessibilityBridgeHealthSnapshot {
    /// Number of pending accessibility tree updates queued for processing
    update_queue_size: usize,
    /// Number of active WebView → egui::Id anchors for tree injection
    anchor_count: usize,
    /// Cumulative count of dropped/lost accessibility updates
    dropped_update_count: usize,
    /// Current focus target in accessibility tree (if any)
    focus_target: Option<String>,
    /// Degradation indicator: "none" | "warning" | "error"
    degradation_state: String,
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphReaderMode {
    Off,
    Room,
    Map,
}

#[cfg(feature = "diagnostics")]
impl GraphReaderMode {
    fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Room => "Room",
            Self::Map => "Map",
        }
    }
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone)]
struct GraphReaderSnapshot {
    mode: GraphReaderMode,
    entry_point_reachable: bool,
    degraded_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "diagnostics")]
    use super::GraphReaderMode;
    use super::{
        GraphshellTileBehavior, PlaintextContent, WryUnavailableReason, decode_plaintext_content,
    };
    use crate::app::GraphViewId;
    use crate::graph::NodeKey;
    use crate::shell::desktop::tests::harness::TestRegistry;
    use crate::shell::desktop::workbench::pane_model::{
        GraphPaneRef, NodePaneState, ToolPaneRef, ToolPaneState,
    };
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use crate::shell::desktop::workbench::ux_tree::{self, UxNodeRole};
    use egui_tiles::{Container, Tile, Tiles};

    #[test]
    fn decode_plaintext_content_returns_text_for_utf8() {
        let bytes = b"# title\nhello world\n";
        let decoded = decode_plaintext_content(bytes);

        match decoded {
            PlaintextContent::Text(text) => {
                assert!(text.contains("title"));
                assert!(text.contains("hello world"));
            }
            PlaintextContent::HexPreview(_) => panic!("expected text decode for utf8 payload"),
        }
    }

    #[test]
    fn decode_plaintext_content_returns_hex_preview_for_binary() {
        let bytes = [0xff, 0x00, 0x81, 0x10, 0x22, 0x33, 0x44, 0x55];
        let decoded = decode_plaintext_content(&bytes);

        match decoded {
            PlaintextContent::Text(_) => panic!("expected hex preview for binary payload"),
            PlaintextContent::HexPreview(hex) => {
                assert!(hex.contains("00000000:"));
                assert!(hex.contains("ff"));
                assert!(hex.contains("81"));
            }
        }
    }

    #[test]
    fn wry_unavailable_reason_maps_to_expected_diagnostics_channel() {
        assert_eq!(
            WryUnavailableReason::FeatureDisabled.diagnostics_channel(),
            crate::shell::desktop::runtime::registries::CHANNEL_VIEWER_FALLBACK_WRY_FEATURE_DISABLED
        );
        assert_eq!(
            WryUnavailableReason::CapabilityMissing.diagnostics_channel(),
            crate::shell::desktop::runtime::registries::CHANNEL_VIEWER_FALLBACK_WRY_CAPABILITY_MISSING
        );
        assert_eq!(
            WryUnavailableReason::DisabledByPreference.diagnostics_channel(),
            crate::shell::desktop::runtime::registries::CHANNEL_VIEWER_FALLBACK_WRY_DISABLED_BY_PREFERENCE
        );
    }

    #[test]
    fn wry_unavailable_reason_exposes_user_facing_messages() {
        assert!(
            WryUnavailableReason::FeatureDisabled
                .message()
                .contains("not compiled")
        );
        assert!(
            WryUnavailableReason::CapabilityMissing
                .message()
                .contains("capability")
        );
        assert!(
            WryUnavailableReason::DisabledByPreference
                .message()
                .contains("disabled")
        );
    }

    #[test]
    fn close_handoff_from_active_node_tab_prefers_right_successor() {
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let node_a = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(NodeKey::new(1))));
        let node_b = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(NodeKey::new(2))));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_a, node_b]);

        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(node_a);
        }

        GraphshellTileBehavior::activate_successor_tab_in_parent_before_close(&mut tiles, node_a);

        let active = match tiles.get(root) {
            Some(Tile::Container(Container::Tabs(tabs))) => tabs.active,
            other => panic!("expected tabs container root, got {other:?}"),
        };
        assert_eq!(active, Some(node_b));
    }

    #[test]
    fn close_handoff_from_active_tool_tab_prefers_left_when_no_right_successor() {
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let tool_tile =
            tiles.insert_pane(TileKind::Tool(ToolPaneRef::new(ToolPaneState::Diagnostics)));
        let root = tiles.insert_tab_tile(vec![graph_tile, tool_tile]);

        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(tool_tile);
        }

        GraphshellTileBehavior::activate_successor_tab_in_parent_before_close(
            &mut tiles, tool_tile,
        );

        let active = match tiles.get(root) {
            Some(Tile::Container(Container::Tabs(tabs))) => tabs.active,
            other => panic!("expected tabs container root, got {other:?}"),
        };
        assert_eq!(active, Some(graph_tile));
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn accessibility_inspector_snapshot_reports_selected_node_profile() {
        use euclid::default::Point2D;
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let snapshot = GraphshellTileBehavior::accessibility_inspector_snapshot(&app);
        assert_eq!(snapshot.total_nodes, 1);
        assert_eq!(snapshot.selected_node_count, 1);

        let selected = snapshot
            .selected_node
            .expect("selected node snapshot expected");
        assert_eq!(selected.node_key, key);
        assert_eq!(selected.node_url, "https://example.com");
        assert!(!selected.viewer_id.is_empty());
        assert!(selected.affordance_projection.is_none());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn accessibility_affordance_projection_maps_focus_selection_and_runtime_blocked() {
        let node_key = NodeKey::new(42);
        let annotations = vec![crate::shell::desktop::workbench::tile_compositor::TileAffordanceAnnotation {
            node_key,
            focus_ring_rendered: true,
            selection_ring_rendered: true,
            lifecycle_treatment:
                crate::shell::desktop::workbench::tile_compositor::LifecycleTreatment::RuntimeBlocked,
            lens_glyphs_rendered: vec!["semantic".to_string()],
            paint_callback_registered: true,
        }];

        let projection =
            crate::shell::desktop::ui::gui::selected_node_affordance_projection_from_annotations(
                node_key,
                &annotations,
            )
            .expect("projection expected");

        assert!(projection.focus_annotation);
        assert!(projection.selection_annotation);
        assert!(projection.aria_busy);
        assert_eq!(projection.lifecycle_label, "runtime-blocked");
        assert_eq!(
            projection.status_tokens,
            vec![
                "focused".to_string(),
                "selected".to_string(),
                "runtime-blocked".to_string(),
                "aria-busy".to_string(),
            ]
        );
        assert_eq!(projection.glyph_descriptions, vec!["semantic".to_string()]);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn accessibility_bridge_health_snapshot_captures_health_metrics() {
        let app = crate::app::GraphBrowserApp::new_for_testing();

        let health = GraphshellTileBehavior::accessibility_bridge_health_snapshot(&app);

        // Verify snapshot structure contains all required health diagnostic fields
        assert_eq!(health.update_queue_size, 0);
        assert_eq!(health.anchor_count, 0);
        assert_eq!(health.dropped_update_count, 0);
        assert_eq!(health.focus_target, None);
        assert_eq!(health.degradation_state, "none");
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn graph_reader_snapshot_exposes_reachable_degraded_entry_point() {
        let app = crate::app::GraphBrowserApp::new_for_testing();

        let snapshot = GraphshellTileBehavior::graph_reader_snapshot(&app);

        assert_eq!(snapshot.mode, GraphReaderMode::Off);
        assert!(snapshot.entry_point_reachable);
        assert!(snapshot.degraded_reason.is_some());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn graph_reader_snapshot_reports_map_mode_when_graph_has_nodes() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let _node = app.add_node_and_sync(
            "https://graph-reader-map.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let snapshot = GraphshellTileBehavior::graph_reader_snapshot(&app);

        assert_eq!(snapshot.mode, GraphReaderMode::Map);
        assert!(snapshot.entry_point_reachable);
        assert!(
            snapshot
                .degraded_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("deterministic Map output"))
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn graph_reader_snapshot_reports_room_mode_when_selected_node_exists() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://graph-reader-room.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let view_id = GraphViewId::new();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.set_workspace_focused_view_with_transition(Some(view_id));
        app.select_node(node, false);

        let snapshot = GraphshellTileBehavior::graph_reader_snapshot(&app);

        assert_eq!(snapshot.mode, GraphReaderMode::Room);
        assert!(snapshot.entry_point_reachable);
        assert!(
            snapshot
                .degraded_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("focused Room grouping"))
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn graph_reader_snapshot_reports_map_mode_when_override_returns_to_map() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://graph-reader-map-return.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let view_id = GraphViewId::new();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.set_workspace_focused_view_with_transition(Some(view_id));
        app.graph_reader_enter_room(node);
        app.graph_reader_return_to_map();

        let snapshot = GraphshellTileBehavior::graph_reader_snapshot(&app);

        assert_eq!(snapshot.mode, GraphReaderMode::Map);
        assert!(snapshot.entry_point_reachable);
    }

    #[test]
    fn uxtree_snapshot_reports_roots_and_selection_projection() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://uxtree.example");
        harness.open_node_tab(node);
        harness.app.select_node(node, false);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let graph_surface_count = snapshot
            .semantic_nodes
            .iter()
            .filter(|entry| entry.role == UxNodeRole::GraphSurface)
            .count();
        let graph_node_count = snapshot
            .semantic_nodes
            .iter()
            .filter(|entry| entry.role == UxNodeRole::GraphNode)
            .count();

        assert!(
            graph_surface_count > 0,
            "expected at least one graph surface semantic node"
        );
        assert_eq!(
            graph_node_count, 1,
            "expected graph semantic parity for one graph node"
        );
    }

    #[test]
    fn uxtree_probe_returns_no_violation_for_minimal_healthy_state() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://uxtree-probe.example");
        harness.open_node_tab(node);
        harness.app.select_node(node, false);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 8);
        let violation = ux_tree::presentation_id_consistency_violation(&snapshot);
        assert!(
            violation.is_none(),
            "healthy uxtree projection should not violate probe invariant"
        );
    }
}
