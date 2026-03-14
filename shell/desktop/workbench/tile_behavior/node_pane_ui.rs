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
    render_node_viewer_backend_selector(ui, behavior.graph_app, state);
    ui.add_space(4.0);

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
    } else {
        let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        log::debug!(
            "tile_behavior: allocated compositor space for node viewer {:?} at {:?}",
            node_key,
            rect
        );
    }
}
