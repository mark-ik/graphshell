use super::*;

impl<'a> GraphshellTileBehavior<'a> {
    pub(super) fn render_graph_pane(
        &mut self,
        ui: &mut egui::Ui,
        view_id: crate::app::GraphViewId,
    ) {
        let pane_rect = ui.max_rect();
        let actions = render::render_graph_in_ui_collect_actions(
            ui,
            self.graph_app,
            view_id,
            self.search_matches,
            self.active_search_match,
            if self.search_filter_mode {
                SearchDisplayMode::Filter
            } else {
                SearchDisplayMode::Highlight
            },
            self.search_query_active,
        );
        let multi_select_modifier = ui.input(|i| i.modifiers.ctrl);
        let mut passthrough_actions = Vec::new();

        for action in actions {
            match action {
                GraphAction::FocusNode(key) => {
                    log::debug!("tile_behavior: FocusNode action for {:?}", key);
                    self.queue_post_render_intent(GraphIntent::OpenNodeFrameRouted {
                        key,
                        prefer_frame: None,
                    });
                }
                GraphAction::FocusNodeSplit(key) => {
                    if let Some(primary) = self.graph_app.focused_selection().primary()
                        && primary != key
                    {
                        self.queue_post_render_intent(GraphIntent::CreateUserGroupedEdge {
                            from: primary,
                            to: key,
                            label: None,
                        });
                    }
                    self.queue_post_render_intent(GraphIntent::SelectNode {
                        key,
                        multi_select: multi_select_modifier,
                    });
                    log::debug!("tile_behavior: enqueue pending open node {:?} split", key);
                    self.pending_open_nodes.push(PendingOpenNode {
                        key,
                        mode: PendingOpenMode::SplitHorizontal,
                    });
                }
                other => passthrough_actions.push(other),
            }
        }

        self.extend_post_render_intents(render::intents_from_graph_actions(passthrough_actions));
        render::sync_graph_positions_from_layout(self.graph_app);
        render::render_graph_info_in_ui(ui, self.graph_app, view_id);
        render_graph_pane_overlay(
            ui.ctx(),
            self.graph_app,
            view_id,
            pane_rect,
            &mut self.pending_post_render_intents,
        );
    }

    pub(super) fn render_node_pane(&mut self, ui: &mut egui::Ui, state: &mut NodePaneState) {
        render_node_pane_impl(self, ui, state);
    }
}

fn render_node_pane_impl(
    behavior: &mut GraphshellTileBehavior<'_>,
    ui: &mut egui::Ui,
    state: &mut NodePaneState,
) {
    let node_key = state.node;
    let Some((node_url, node_mime_hint, node_lifecycle)) = behavior
        .graph_app
        .domain_graph()
        .get_node(node_key)
        .map(|node| (node.url.clone(), node.mime_hint.clone(), node.lifecycle))
    else {
        ui.label("Missing node for this tile.");
        return;
    };
    // Suppress the in-pane viewer backend selector when a native overlay (e.g. wry) is active:
    // the overlay covers this area so egui controls would be unreachable. The graph bar's
    // "Compat"/"Servo" button is the accessible control path for native overlay panes.
    if state.render_mode
        != crate::shell::desktop::workbench::pane_model::TileRenderMode::NativeOverlay
    {
        render_node_viewer_backend_selector(ui, behavior.graph_app, state);
        ui.add_space(4.0);
    }

    let effective_viewer_id = state
        .viewer_id_override
        .as_ref()
        .map(|viewer_id| viewer_id.as_str().to_string())
        .unwrap_or_else(|| {
            crate::shell::desktop::runtime::registries::phase0_select_viewer_for_content(
                &node_url,
                node_mime_hint.as_deref(),
            )
            .viewer_id
            .to_string()
        });

    if effective_viewer_id.as_str() == "viewer:settings" {
        match GraphBrowserApp::resolve_settings_route(&node_url) {
            Some(crate::app::SettingsRouteTarget::Settings(page)) => {
                behavior.graph_app.workspace.chrome_ui.settings_tool_page = page;
                let intents = render::render_settings_node_viewer_in_ui(ui, behavior.graph_app);
                behavior.extend_post_render_intents(intents);
            }
            Some(crate::app::SettingsRouteTarget::History) => {
                let intents = render::render_history_manager_in_ui(ui, behavior.graph_app);
                behavior.extend_post_render_intents(intents);
            }
            None => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 180, 60),
                    "Settings route unresolved",
                );
                ui.label(format!(
                    "No settings page mapping exists for '{}'.",
                    node_url
                ));
                ui.horizontal(|ui| {
                    if ui.button("Open Settings Pane").clicked() {
                        behavior
                            .graph_app
                            .enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                                kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings,
                            });
                    }
                    if ui.button("Use WebView Fallback").clicked() {
                        request_viewer_backend_swap(
                            behavior.graph_app,
                            state,
                            Some(ViewerId::new("viewer:webview")),
                        );
                    }
                });
            }
        }
        return;
    }

    if matches!(
        effective_viewer_id.as_str(),
        "viewer:plaintext" | "viewer:markdown"
    ) {
        ui.label(format!("{}", node_url));
        ui.separator();
        match load_plaintext_content_for_node(&node_url) {
            Ok(PlaintextContent::Text(content)) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if effective_viewer_id.as_str() == "viewer:markdown" {
                        render_markdown_embedded(ui, &content);
                    } else {
                        let mut read_only = content;
                        ui.add(
                            egui::TextEdit::multiline(&mut read_only)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    }
                });
            }
            Ok(PlaintextContent::HexPreview(hex)) => {
                ui.small("Binary content detected; showing hex preview.");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut read_only = hex;
                    ui.add(
                        egui::TextEdit::multiline(&mut read_only)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
            }
            Err(error) => {
                ui.small(error);
            }
        }
        return;
    }

    if !tile_runtime::viewer_id_uses_composited_runtime(effective_viewer_id.as_str()) {
        if effective_viewer_id.as_str() == "viewer:wry"
            && state.render_mode
                == crate::shell::desktop::workbench::pane_model::TileRenderMode::NativeOverlay
        {
            if let Some(reason) = wry_unavailable_reason(behavior.graph_app) {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: reason.diagnostics_channel(),
                    byte_len: 1,
                });
                ui.colored_label(
                    egui::Color32::from_rgb(220, 180, 60),
                    "Wry backend currently unavailable",
                );
                ui.label(reason.message());
                ui.horizontal(|ui| {
                    if ui.button("Use WebView").clicked() {
                        request_viewer_backend_swap(
                            behavior.graph_app,
                            state,
                            Some(ViewerId::new("viewer:webview")),
                        );
                    }
                    if ui.button("Clear Viewer Override").clicked() {
                        request_viewer_backend_swap(behavior.graph_app, state, None);
                    }
                });
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(130, 185, 130),
                    "Wry native overlay active",
                );
                ui.small(
                    "This pane is rendered through native overlay sync (not composited texture).",
                );
            }
            ui.small(format!("URL: {}", node_url));
            return;
        }

        let is_placeholder_mode = matches!(
            state.render_mode,
            crate::shell::desktop::workbench::pane_model::TileRenderMode::Placeholder
        );
        if is_placeholder_mode {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
                byte_len: effective_viewer_id.len(),
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_VIEWER_FALLBACK_USED,
                byte_len: effective_viewer_id.len(),
            });
            ui.colored_label(
                egui::Color32::from_rgb(220, 180, 60),
                "Viewer fallback active (placeholder mode)",
            );
            ui.label(format!(
                "Reason: '{}' is unresolved for this build path and falls back to placeholder rendering.",
                effective_viewer_id
            ));
            ui.small("Recovery: switch this pane to WebView fallback or clear the override.");
            ui.horizontal(|ui| {
                if ui.button("Use WebView Fallback").clicked() {
                    request_viewer_backend_swap(
                        behavior.graph_app,
                        state,
                        Some(ViewerId::new("viewer:webview")),
                    );
                }
                if ui.button("Clear Viewer Override").clicked() {
                    request_viewer_backend_swap(behavior.graph_app, state, None);
                }
            });
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(220, 180, 60),
                "Viewer path is currently degraded",
            );
            ui.label(format!(
                "Reason: '{}' is not rendered through this pane path yet.",
                effective_viewer_id
            ));
            ui.small("Recovery: use a supported embedded viewer or switch to WebView.");
            ui.horizontal(|ui| {
                if ui.button("Use WebView").clicked() {
                    request_viewer_backend_swap(
                        behavior.graph_app,
                        state,
                        Some(ViewerId::new("viewer:webview")),
                    );
                }
                if ui.button("Clear Viewer Override").clicked() {
                    request_viewer_backend_swap(behavior.graph_app, state, None);
                }
            });
        }
        ui.small(format!("URL: {}", node_url));
        return;
    }

    if let Some(crash) = behavior
        .graph_app
        .runtime_crash_state_for_node(node_key)
        .cloned()
    {
        let crash_reason = crash.message.as_deref().unwrap_or("unknown");
        ui.colored_label(
            egui::Color32::from_rgb(220, 120, 120),
            format!("Tab crashed: {}", crash_reason),
        );
        ui.horizontal(|ui| {
            if ui.button("Reload").clicked() {
                behavior.queue_post_render_intent(lifecycle_intents::promote_node_to_active(
                    node_key,
                    LifecycleCause::UserSelect,
                ));
            }
            if ui.button("Close Tile").clicked() {
                behavior
                    .graph_app
                    .demote_node_to_cold_with_cause(node_key, LifecycleCause::ExplicitClose);
                behavior.pending_closed_nodes.push(node_key);
            }
        });
        if crash.has_backtrace {
            ui.small("Crash reported a backtrace.");
        }
        if let Ok(elapsed) = std::time::SystemTime::now().duration_since(crash.blocked_at) {
            ui.small(format!("Crashed {}s ago", elapsed.as_secs()));
        }
        return;
    }

    if behavior.graph_app.get_webview_for_node(node_key).is_none() {
        log::debug!(
            "tile_behavior: node {:?} has no active node viewer runtime",
            node_key
        );
        let block_state = behavior
            .graph_app
            .runtime_block_state_for_node(node_key)
            .cloned();
        let lifecycle_hint = match node_lifecycle {
            NodeLifecycle::Cold => "Node is cold. Reactivate to resume browsing in this pane.",
            NodeLifecycle::Warm => {
                "Node is warm-cached. Reactivate to attach its cached runtime viewer."
            }
            NodeLifecycle::Active => "Node is active but no runtime viewer is mapped yet.",
            NodeLifecycle::Tombstone => {
                "Node is tombstoned and is retained for history continuity."
            }
        };
        if let Some(block_state) = block_state {
            ui.colored_label(
                egui::Color32::from_rgb(220, 180, 60),
                "Degraded: runtime viewer currently blocked",
            );
            let reason = match block_state.reason {
                crate::app::RuntimeBlockReason::CreateRetryExhausted => {
                    "WebView creation retries were exhausted and a cooldown is active."
                }
                crate::app::RuntimeBlockReason::Crash => {
                    "Viewer crashed and runtime is temporarily blocked."
                }
            };
            ui.label(format!("Reason: {reason}"));
            if let Some(retry_at) = block_state.retry_at {
                let now = std::time::Instant::now();
                if retry_at > now {
                    ui.small(format!(
                        "Recovery: retry available in ~{}s.",
                        retry_at.duration_since(now).as_secs()
                    ));
                }
            }
        }

        ui.label(format!("No active runtime viewer for {}", node_url));
        ui.small(lifecycle_hint);
        ui.horizontal(|ui| {
            if ui.button("Reactivate").clicked() {
                behavior.queue_post_render_intent(GraphIntent::SelectNode {
                    key: node_key,
                    multi_select: false,
                });
                behavior.queue_post_render_intent(GraphIntent::SelectNode {
                    key: node_key,
                    multi_select: false,
                });
                behavior.queue_post_render_intent(lifecycle_intents::promote_node_to_active(
                    node_key,
                    LifecycleCause::UserSelect,
                ));
            }
        });
        ui.add_space(8.0);
        render_node_history_panel(behavior, ui, state, node_key);
        render_node_audit_panel(behavior, ui, state, node_key);
    } else {
        let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        log::debug!(
            "tile_behavior: allocated compositor space for node viewer {:?} at {:?}",
            node_key,
            rect
        );
    }
}

fn render_node_history_panel(
    behavior: &mut GraphshellTileBehavior<'_>,
    ui: &mut egui::Ui,
    state: &mut NodePaneState,
    node_key: NodeKey,
) {
    use crate::services::persistence::types::{LogEntry, PersistedNavigationTrigger};
    use std::time::{SystemTime, UNIX_EPOCH};

    let node_id = match behavior.graph_app.domain_graph().get_node(node_key) {
        Some(node) => node.id,
        None => return,
    };

    let header_label = if state.show_node_history {
        "▼ Node History"
    } else {
        "▶ Node History"
    };
    if ui.small_button(header_label).clicked() {
        state.show_node_history = !state.show_node_history;
    }

    if !state.show_node_history {
        return;
    }

    const LIMIT: usize = 50;
    let entries = behavior
        .graph_app
        .node_navigation_history_entries(node_id, LIMIT);

    if entries.is_empty() {
        ui.small("No navigation history for this node.");
        return;
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for entry in &entries {
                let LogEntry::NavigateNode {
                    to_url,
                    from_url,
                    trigger,
                    timestamp_ms,
                    ..
                } = entry
                else {
                    continue;
                };

                let elapsed_ms = now_ms.saturating_sub(*timestamp_ms);
                let time_label = if elapsed_ms < 1_000 {
                    "just now".to_string()
                } else if elapsed_ms < 60_000 {
                    format!("{}s ago", elapsed_ms / 1_000)
                } else if elapsed_ms < 3_600_000 {
                    format!("{}m ago", elapsed_ms / 60_000)
                } else if elapsed_ms < 86_400_000 {
                    format!("{}h ago", elapsed_ms / 3_600_000)
                } else {
                    format!("{}d ago", elapsed_ms / 86_400_000)
                };

                let trigger_icon = match trigger {
                    PersistedNavigationTrigger::LinkClick => "🔗",
                    PersistedNavigationTrigger::Back => "⬅",
                    PersistedNavigationTrigger::Forward => "➡",
                    PersistedNavigationTrigger::AddressBarEntry => "⌨",
                    PersistedNavigationTrigger::PanePromotion => "⬆",
                    PersistedNavigationTrigger::Programmatic => "⚙",
                    PersistedNavigationTrigger::Unknown => "↔",
                };

                let from_short = truncate_host_or_path(from_url, 28);
                let to_short = truncate_host_or_path(to_url, 28);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&time_label).weak().small());
                    ui.label(trigger_icon);
                    let response =
                        ui.selectable_label(false, format!("{} → {}", from_short, to_short));
                    if response.clicked() {
                        behavior.queue_post_render_intent(GraphIntent::SetNodeUrl {
                            key: node_key,
                            new_url: to_url.clone(),
                        });
                    }
                });
            }
        });
}

fn render_node_audit_panel(
    behavior: &mut GraphshellTileBehavior<'_>,
    ui: &mut egui::Ui,
    state: &mut NodePaneState,
    node_key: NodeKey,
) {
    use crate::services::persistence::types::{LogEntry, NodeAuditEventKind};
    use std::time::{SystemTime, UNIX_EPOCH};

    let node_id = match behavior.graph_app.domain_graph().get_node(node_key) {
        Some(node) => node.id,
        None => return,
    };

    let header_label = if state.show_node_audit {
        "▼ Node Audit"
    } else {
        "▶ Node Audit"
    };
    if ui.small_button(header_label).clicked() {
        state.show_node_audit = !state.show_node_audit;
    }

    if !state.show_node_audit {
        return;
    }

    const LIMIT: usize = 50;
    let entries = behavior
        .graph_app
        .node_audit_history_entries(node_id, LIMIT);

    if entries.is_empty() {
        ui.small("No audit events for this node.");
        return;
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for entry in &entries {
                let LogEntry::AppendNodeAuditEvent {
                    event,
                    timestamp_ms,
                    ..
                } = entry
                else {
                    continue;
                };

                let elapsed_ms = now_ms.saturating_sub(*timestamp_ms);
                let time_label = if elapsed_ms < 1_000 {
                    "just now".to_string()
                } else if elapsed_ms < 60_000 {
                    format!("{}s ago", elapsed_ms / 1_000)
                } else if elapsed_ms < 3_600_000 {
                    format!("{}m ago", elapsed_ms / 60_000)
                } else if elapsed_ms < 86_400_000 {
                    format!("{}h ago", elapsed_ms / 3_600_000)
                } else {
                    format!("{}d ago", elapsed_ms / 86_400_000)
                };

                let (icon, description) = match event {
                    NodeAuditEventKind::TitleChanged { new_title } => (
                        "✏",
                        format!("Renamed to \"{}\"", truncate_host_or_path(new_title, 32)),
                    ),
                    NodeAuditEventKind::Tagged { tag } => ("🏷", format!("Tagged: {}", tag)),
                    NodeAuditEventKind::Untagged { tag } => ("🏷", format!("Untagged: {}", tag)),
                    NodeAuditEventKind::Pinned => ("📌", "Pinned".to_string()),
                    NodeAuditEventKind::Unpinned => ("📌", "Unpinned".to_string()),
                    NodeAuditEventKind::UrlChanged { new_url } => (
                        "🔗",
                        format!("URL → {}", truncate_host_or_path(new_url, 32)),
                    ),
                    NodeAuditEventKind::Tombstoned => ("🪦", "Tombstoned".to_string()),
                    NodeAuditEventKind::Restored => ("♻", "Restored".to_string()),
                };

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&time_label).weak().small());
                    ui.label(icon);
                    ui.label(egui::RichText::new(&description).small());
                });
            }
        });
}

/// Shorten a URL to hostname + truncated path for display.
fn truncate_host_or_path(url: &str, max_len: usize) -> String {
    let display = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    if display.len() <= max_len {
        display.to_string()
    } else {
        format!("{}…", &display[..max_len.saturating_sub(1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    use crate::app::SettingsToolPage;
    use crate::shell::desktop::runtime::control_panel::ControlPanel;
    use crate::util::{GraphshellSettingsPath, VersoAddress};

    #[test]
    fn viewer_settings_route_renders_embedded_settings_surface_and_updates_page() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            VersoAddress::settings(GraphshellSettingsPath::Physics).to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.workspace.chrome_ui.settings_tool_page = SettingsToolPage::General;

        let mut state = NodePaneState::for_node(node_key);
        state.viewer_id_override = Some(ViewerId::new("viewer:settings"));

        let mut control_panel = ControlPanel::new(None);
        let mut tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)> =
            HashMap::new();
        let search_matches = HashSet::new();

        #[cfg(feature = "diagnostics")]
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |_ctx| {
            egui::CentralPanel::default().show(_ctx, |ui| {
                let mut behavior = GraphshellTileBehavior::new(
                    &mut app,
                    &mut control_panel,
                    &mut tile_favicon_textures,
                    &search_matches,
                    None,
                    false,
                    false,
                    #[cfg(feature = "diagnostics")]
                    &mut diagnostics,
                    #[cfg(feature = "diagnostics")]
                    None,
                );
                behavior.render_node_pane(ui, &mut state);
            });
        });

        assert_eq!(
            app.workspace.chrome_ui.settings_tool_page,
            SettingsToolPage::Physics
        );
    }
}
