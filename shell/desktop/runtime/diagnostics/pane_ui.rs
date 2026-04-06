use super::*;

impl DiagnosticsState {
    fn render_engine_topology(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(520.0, 260.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(12, 16, 24));
        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 60, 90)),
            egui::StrokeKind::Inside,
        );

        let servo_runtime = egui::pos2(rect.left() + 90.0, rect.top() + 35.0);
        let semantic = egui::pos2(rect.left() + 90.0, rect.top() + 90.0);
        let intents = egui::pos2(rect.left() + 90.0, rect.bottom() - 55.0);
        let render_pass = egui::pos2(rect.center().x, rect.center().y);
        let compositor = egui::pos2(rect.right() - 100.0, rect.top() + 85.0);
        let backpressure = egui::pos2(rect.right() - 100.0, rect.bottom() - 85.0);

        let draw_edge =
            |from: egui::Pos2, to: egui::Pos2, metric: EdgeMetric, p: &egui::Painter| {
                let t = (metric.count as f32).ln_1p().clamp(0.0, 4.0);
                let width = 1.0 + t;
                let color = if metric.bottleneck {
                    egui::Color32::from_rgb(255, 100, 100)
                } else if metric.count > 0 {
                    egui::Color32::from_rgb(80, 240, 200)
                } else {
                    egui::Color32::from_rgb(60, 80, 90)
                };
                p.line_segment([from, to], egui::Stroke::new(width, color));
                let mid = egui::pos2((from.x + to.x) * 0.5, (from.y + to.y) * 0.5 - 8.0);
                p.text(
                    mid,
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "{} | {} {:.1}ms",
                        metric.count,
                        self.latency_percentile.label(),
                        metric.percentile_latency_us as f64 / 1000.0
                    ),
                    egui::FontId::monospace(11.0),
                    color,
                );
            };

        draw_edge(
            servo_runtime,
            semantic,
            self.edge_metric(&CHANNELS_SERVO_TO_SEMANTIC),
            &painter,
        );
        draw_edge(
            semantic,
            intents,
            self.edge_metric(&CHANNELS_SEMANTIC_TO_INTENTS),
            &painter,
        );
        draw_edge(
            intents,
            render_pass,
            self.edge_metric(&CHANNELS_INTENTS_TO_RENDER_PASS),
            &painter,
        );
        draw_edge(
            render_pass,
            compositor,
            self.edge_metric(&CHANNELS_RENDER_PASS_TO_COMPOSITOR),
            &painter,
        );
        draw_edge(
            backpressure,
            intents,
            self.edge_metric(&CHANNELS_BACKPRESSURE_TO_INTENTS),
            &painter,
        );
        draw_edge(
            intents,
            compositor,
            self.edge_metric(&CHANNELS_INTENTS_TO_COMPOSITOR),
            &painter,
        );

        let draw_node = |center: egui::Pos2, label: &str, p: &egui::Painter| {
            let node_rect = egui::Rect::from_center_size(center, egui::vec2(110.0, 36.0));
            p.rect_filled(node_rect, 5.0, egui::Color32::from_rgb(20, 30, 46));
            p.rect_stroke(
                node_rect,
                5.0,
                egui::Stroke::new(1.2, egui::Color32::from_rgb(90, 220, 255)),
                egui::StrokeKind::Inside,
            );
            p.text(
                node_rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(13.0),
                egui::Color32::from_rgb(210, 240, 255),
            );
        };

        draw_node(servo_runtime, "Servo Runtime", &painter);
        draw_node(semantic, "Semantic Ingress", &painter);
        draw_node(intents, "Intent Pipeline", &painter);
        draw_node(render_pass, "Render Pass", &painter);
        draw_node(compositor, "Compositor", &painter);
        draw_node(backpressure, "Backpressure", &painter);
    }

    fn render_focus_inspector(ui: &mut egui::Ui, focus_inspector: Option<&RuntimeFocusInspector>) {
        let Some(focus_inspector) = focus_inspector else {
            ui.small("focus inspector unavailable");
            return;
        };

        egui::CollapsingHeader::new("Focus Inspector")
            .default_open(true)
            .show(ui, |ui| {
                ui.small("Desired vs realized runtime focus");
                egui::Grid::new("diag_focus_inspector_grid")
                    .num_columns(2)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Desired");
                        ui.strong("Realized");
                        ui.end_row();

                        ui.monospace(format!("{:#?}", focus_inspector.desired));
                        ui.monospace(format!("{:#?}", focus_inspector.realized));
                        ui.end_row();
                    });
            });
    }

    pub(crate) fn render_in_pane(
        &mut self,
        ui: &mut egui::Ui,
        graph_app: &mut GraphBrowserApp,
        focus_inspector: Option<&RuntimeFocusInspector>,
        signal_trace: &[crate::shell::desktop::runtime::registries::signal_routing::SignalTraceEntry],
    ) {
        self.sync_persistence_health_snapshot_from_app(graph_app);
        self.sync_history_health_snapshot_from_app(graph_app);
        self.sync_security_health_snapshot_from_runtime();
        self.sync_runtime_cache_snapshot_from_app(graph_app);
        self.sync_tracing_perf_snapshot_from_runtime();
        self.tick_drain();
        self.hovered_node_key = None;

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, DiagnosticsTab::Engine, "Engine");
            ui.selectable_value(&mut self.active_tab, DiagnosticsTab::Analysis, "Analysis");
            ui.selectable_value(
                &mut self.active_tab,
                DiagnosticsTab::Compositor,
                "Compositor",
            );
            ui.selectable_value(&mut self.active_tab, DiagnosticsTab::Intents, "Intents");
        });
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Save Snapshot JSON").clicked() {
                match self.export_snapshot_json() {
                    Ok(path) => {
                        let replay_samples = replay_samples_snapshot();
                        log::info!("Diagnostics JSON exported: {}", path.display());
                        self.export_feedback =
                            Some(Self::replay_export_feedback(&path, &replay_samples));
                    }
                    Err(err) => {
                        log::warn!("Diagnostics JSON export failed: {err}");
                        self.export_feedback = Some(format!("JSON export failed: {err}"));
                    }
                }
            }
            if ui.button("Save Snapshot SVG").clicked() {
                match self.export_snapshot_svg() {
                    Ok(path) => {
                        log::info!("Diagnostics SVG exported: {}", path.display());
                        self.export_feedback = Some(format!("Saved SVG: {}", path.display()));
                    }
                    Err(err) => {
                        log::warn!("Diagnostics SVG export failed: {err}");
                        self.export_feedback = Some(format!("SVG export failed: {err}"));
                    }
                }
            }
            if ui.button("Save Bridge Spike JSON").clicked() {
                match self.export_bridge_spike_json() {
                    Ok(path) => {
                        log::info!("Bridge spike JSON exported: {}", path.display());
                        self.export_feedback =
                            Some(format!("Saved Bridge Spike JSON: {}", path.display()));
                    }
                    Err(err) => {
                        log::warn!("Bridge spike JSON export failed: {err}");
                        self.export_feedback =
                            Some(format!("Bridge Spike JSON export failed: {err}"));
                    }
                }
            }
            if ui.button("Save Backend Telemetry JSON").clicked() {
                match self.export_backend_telemetry_report_json() {
                    Ok(path) => {
                        log::info!("Backend telemetry JSON exported: {}", path.display());
                        self.export_feedback =
                            Some(format!("Saved Backend Telemetry JSON: {}", path.display()));
                    }
                    Err(err) => {
                        log::warn!("Backend telemetry JSON export failed: {err}");
                        self.export_feedback =
                            Some(format!("Backend Telemetry JSON export failed: {err}"));
                    }
                }
            }
        });
        if let Some(feedback) = &self.export_feedback {
            ui.small(feedback);
        }
        ui.separator();

        match self.active_tab {
            DiagnosticsTab::Engine => {
                ui.label("Engine topology inspector");
                let active = self
                    .compositor_state
                    .frames
                    .back()
                    .map(|f| f.active_tile_count)
                    .unwrap_or(0);
                ui.small(format!("Active composited tiles: {active}"));
                ui.small(format!(
                    "event_ring={} channels={} spans={}",
                    self.event_ring.len(),
                    self.diagnostic_graph.message_counts.len(),
                    self.diagnostic_graph.last_span_duration_us.len()
                ));
                let history_preview = self.history_health_snapshot["preview_mode_active"]
                    .as_bool()
                    .unwrap_or(false);
                let history_failures =
                    self.history_health_snapshot["recent_traversal_append_failures"]
                        .as_u64()
                        .unwrap_or(0);
                ui.small(format!(
                    "persistence_health: store={} recovery={} layouts={} snapshots={}",
                    self.persistence_health_snapshot["store_status"]
                        .as_str()
                        .unwrap_or("unknown"),
                    self.persistence_health_snapshot["recovery_status"]
                        .as_str()
                        .unwrap_or("unknown"),
                    self.persistence_health_snapshot["workspace_layout_count"]
                        .as_u64()
                        .unwrap_or(0),
                    self.persistence_health_snapshot["named_graph_snapshot_count"]
                        .as_u64()
                        .unwrap_or(0)
                ));
                ui.small(format!(
                    "history_health: preview_active={} failures={}",
                    history_preview, history_failures
                ));
                let trusted_peer_count = self.security_health_snapshot["trusted_peer_count"]
                    .as_u64()
                    .unwrap_or(0);
                let workspace_grant_count = self.security_health_snapshot["workspace_grant_count"]
                    .as_u64()
                    .unwrap_or(0);
                let access_denied_count = self.security_health_snapshot["access_denied_count"]
                    .as_u64()
                    .unwrap_or(0);
                ui.small(format!(
                    "security_health: peers={} grants={} access_denied={}",
                    trusted_peer_count, workspace_grant_count, access_denied_count
                ));
                let cache_hits = self.runtime_cache_snapshot["hits"].as_u64().unwrap_or(0);
                let cache_misses = self.runtime_cache_snapshot["misses"].as_u64().unwrap_or(0);
                let cache_inserts = self.runtime_cache_snapshot["inserts"].as_u64().unwrap_or(0);
                let cache_evictions = self.runtime_cache_snapshot["evictions"]
                    .as_u64()
                    .unwrap_or(0);
                ui.small(format!(
                    "runtime_cache: hits={} misses={} inserts={} evictions={}",
                    cache_hits, cache_misses, cache_inserts, cache_evictions
                ));
                let perf_sample_count = self.tracing_perf_snapshot["sample_count"]
                    .as_u64()
                    .unwrap_or(0);
                let perf_avg_us = self.tracing_perf_snapshot["avg_elapsed_us"]
                    .as_u64()
                    .unwrap_or(0);
                let perf_p95_us = self.tracing_perf_snapshot["p95_elapsed_us"]
                    .as_u64()
                    .unwrap_or(0);
                let perf_max_us = self.tracing_perf_snapshot["max_elapsed_us"]
                    .as_u64()
                    .unwrap_or(0);
                ui.small(format!(
                    "tracing_perf: samples={} avg={}us p95={}us max={}us",
                    perf_sample_count, perf_avg_us, perf_p95_us, perf_max_us
                ));
                let analyzer_snapshots = self.analyzer_snapshots();
                if let Some(hotpath) = analyzer_snapshots
                    .iter()
                    .find(|snapshot| snapshot.id == "tracing.hotpath.latency")
                {
                    let signal = hotpath
                        .last_result
                        .as_ref()
                        .map(|result| result.signal)
                        .unwrap_or(AnalyzerSignal::Quiet);
                    let signal_label = match signal {
                        AnalyzerSignal::Quiet => "quiet",
                        AnalyzerSignal::Active => "active",
                        AnalyzerSignal::Alert => "alert",
                    };
                    ui.small(format!("tracing_hotpath_analyzer: {signal_label}"));
                }
                if !analyzer_snapshots.is_empty() {
                    egui::CollapsingHeader::new("Active analyzers")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.small("Analyzer registry moved to the Analysis tab.");
                        });
                }
                let active_tile_violations = self.channel_count(CHANNEL_ACTIVE_TILE_VIOLATION);
                if active_tile_violations > 0 {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 120, 120),
                        format!(
                            "Active tile violations: {} ({})",
                            active_tile_violations, CHANNEL_ACTIVE_TILE_VIOLATION
                        ),
                    );
                } else {
                    ui.small(format!(
                        "Active tile violations: 0 ({})",
                        CHANNEL_ACTIVE_TILE_VIOLATION
                    ));
                }
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("diag_latency_percentile")
                        .selected_text(self.latency_percentile.label().to_uppercase())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.latency_percentile,
                                LatencyPercentile::P90,
                                "P90",
                            );
                            ui.selectable_value(
                                &mut self.latency_percentile,
                                LatencyPercentile::P95,
                                "P95",
                            );
                            ui.selectable_value(
                                &mut self.latency_percentile,
                                LatencyPercentile::P99,
                                "P99",
                            );
                        });
                    let mut threshold_ms = self.bottleneck_latency_us as f64 / 1000.0;
                    ui.add(
                        egui::Slider::new(&mut threshold_ms, 5.0..=250.0)
                            .text("Bottleneck ms")
                            .step_by(1.0),
                    );
                    self.bottleneck_latency_us = (threshold_ms * 1000.0).round() as u64;
                    ui.small(format!(
                        "Threshold: {:.1}ms {}",
                        self.bottleneck_latency_us as f64 / 1000.0,
                        self.latency_percentile.label()
                    ));
                    if ui.button("Reset metrics").clicked() {
                        self.clear_aggregates();
                    }
                });
                Self::render_focus_inspector(ui, focus_inspector);
                self.render_engine_topology(ui);
                ui.separator();

                let mut channels: Vec<(&'static str, u64, u64, u64)> = self
                    .diagnostic_graph
                    .message_counts
                    .iter()
                    .map(|(channel, count)| {
                        let samples = self
                            .diagnostic_graph
                            .message_latency_samples
                            .get(channel)
                            .copied()
                            .unwrap_or(0);
                        let avg_us = if samples > 0 {
                            self.diagnostic_graph
                                .message_latency_us
                                .get(channel)
                                .copied()
                                .unwrap_or(0)
                                / samples
                        } else {
                            0
                        };
                        let mut recent = self
                            .diagnostic_graph
                            .message_latency_recent_us
                            .get(channel)
                            .map(|latencies| latencies.iter().copied().collect::<Vec<_>>())
                            .unwrap_or_default();
                        let percentile_us = self.selected_percentile_latency_us(&mut recent);
                        (*channel, *count, avg_us, percentile_us)
                    })
                    .collect();
                channels.sort_by(|a, b| b.1.cmp(&a.1));
                ui.label("Hot channels");
                let percentile_header =
                    format!("{} Latency", self.latency_percentile.label().to_uppercase());
                egui::Grid::new("diag_hot_channels")
                    .num_columns(4)
                    .show(ui, |ui| {
                        ui.strong("Channel");
                        ui.strong("Count");
                        ui.strong("Avg Latency");
                        ui.strong(percentile_header);
                        ui.end_row();
                        for (channel, count, avg_us, percentile_us) in channels.into_iter().take(8)
                        {
                            let is_bottleneck = percentile_us >= self.bottleneck_latency_us;
                            if is_bottleneck {
                                ui.colored_label(egui::Color32::from_rgb(255, 120, 120), channel);
                            } else {
                                ui.monospace(channel);
                            }
                            ui.monospace(format!("{count}"));
                            let latency_label = format!("{:.1}ms", avg_us as f64 / 1000.0);
                            if is_bottleneck {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 120, 120),
                                    latency_label,
                                );
                            } else {
                                ui.monospace(latency_label);
                            }
                            let p95_latency_label =
                                format!("{:.1}ms", percentile_us as f64 / 1000.0);
                            if is_bottleneck {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 120, 120),
                                    p95_latency_label,
                                );
                            } else {
                                ui.monospace(p95_latency_label);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                self.render_compositor_overlay_buckets(ui);
                ui.separator();

                let mut channel_configs = diagnostics_registry::list_channel_configs_snapshot();
                channel_configs.sort_by(|a, b| a.0.channel_id.cmp(&b.0.channel_id));
                egui::CollapsingHeader::new("Channel Config Registry")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.small(
                            "Runtime channel controls. Changes apply immediately and persist to workspace settings.",
                        );
                        egui::Grid::new("diag_channel_config_grid")
                            .num_columns(5)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Channel");
                                ui.strong("Enabled");
                                ui.strong("Sample");
                                ui.strong("Retention");
                                ui.strong("Owner");
                                ui.end_row();

                                for (descriptor, mut config) in channel_configs.into_iter().take(80) {
                                    ui.monospace(&descriptor.channel_id);

                                    let mut changed = false;
                                    if ui.checkbox(&mut config.enabled, "").changed() {
                                        changed = true;
                                    }
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut config.sample_rate, 0.0..=1.0)
                                                .show_value(true)
                                                .fixed_decimals(2),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut config.retention_count)
                                                .speed(1)
                                                .range(1..=10_000),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    ui.monospace(format!("{:?}", descriptor.owner.source));
                                    ui.end_row();

                                    if changed {
                                        apply_channel_config_update_with_diagnostics(
                                            self,
                                            graph_app,
                                            &descriptor.channel_id,
                                            config.clone(),
                                        );
                                    }
                                }
                            });
                    });

                let orphan_channels = diagnostics_registry::list_orphan_channels_snapshot();
                egui::CollapsingHeader::new("Orphan Channels")
                    .default_open(false)
                    .show(ui, |ui| {
                        let total_hits: u64 = orphan_channels.iter().map(|(_, count)| *count).sum();
                        ui.small(format!(
                            "Auto-registered runtime channels detected: {} (registration hits: {})",
                            orphan_channels.len(),
                            total_hits
                        ));

                        if orphan_channels.is_empty() {
                            ui.small("No orphan channels detected in this session.");
                        } else {
                            egui::Grid::new("diag_orphan_channel_grid")
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Channel");
                                    ui.strong("Auto-registration hits");
                                    ui.end_row();

                                    for (channel_id, count) in orphan_channels {
                                        ui.monospace(channel_id);
                                        ui.monospace(count.to_string());
                                        ui.end_row();
                                    }
                                });
                        }
                    });

                egui::Grid::new("diag_span_table")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.strong("Span");
                        ui.strong("Last us");
                        ui.end_row();
                        for (name, us) in &self.diagnostic_graph.last_span_duration_us {
                            ui.monospace(*name);
                            ui.monospace(format!("{us}"));
                            ui.end_row();
                        }
                    });
            }
            DiagnosticsTab::Analysis => {
                ui.label("Analyzer and lane-harness inspector");
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.text_edit_singleline(&mut self.analysis_query);
                    ui.checkbox(&mut self.analysis_only_alerts, "Only alerts");
                    if ui.button("Clear").clicked() {
                        self.analysis_query.clear();
                        self.analysis_only_alerts = false;
                    }
                });

                let mut analyzer_snapshots = self
                    .analyzer_snapshots()
                    .into_iter()
                    .filter(|snapshot| {
                        let signal = snapshot
                            .last_result
                            .as_ref()
                            .map(|result| result.signal)
                            .unwrap_or(AnalyzerSignal::Quiet);
                        (!self.analysis_only_alerts || signal == AnalyzerSignal::Alert)
                            && self.analysis_filter_matches(snapshot.id)
                    })
                    .collect::<Vec<_>>();
                analyzer_snapshots.sort_by_key(|snapshot| {
                    (
                        !self.pinned_analyzer_ids.contains(snapshot.id),
                        snapshot.id.to_string(),
                    )
                });

                let lane_summaries = self
                    .lane_channel_summaries()
                    .into_iter()
                    .filter(|lane| {
                        (!self.analysis_only_alerts || lane.signal == AnalyzerSignal::Alert)
                            && self.analysis_filter_matches(lane.label)
                    })
                    .collect::<Vec<_>>();
                let mut channel_trends = self
                    .top_channel_trends(8)
                    .into_iter()
                    .filter(|trend| self.analysis_filter_matches(trend.channel_id))
                    .collect::<Vec<_>>();
                channel_trends.sort_by_key(|trend| {
                    (
                        !self.pinned_channels.contains(trend.channel_id),
                        std::cmp::Reverse(trend.message_count),
                    )
                });
                let channel_histories = self
                    .channel_history_summaries(8)
                    .into_iter()
                    .filter(|history| self.analysis_filter_matches(history.channel_id))
                    .collect::<Vec<_>>();
                let (quiet_count, active_count, alert_count) = self.analyzer_signal_rollup();
                ui.small(format!(
                    "analyzers={} quiet={} active={} alert={}",
                    analyzer_snapshots.len(),
                    quiet_count,
                    active_count,
                    alert_count
                ));
                ui.horizontal_wrapped(|ui| {
                    let draw_chip =
                        |ui: &mut egui::Ui, label: &str, count: usize, color: egui::Color32| {
                            egui::Frame::new()
                                .fill(color.gamma_multiply(0.16))
                                .stroke(egui::Stroke::new(1.0, color))
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.colored_label(color, format!("{label}: {count}"));
                                });
                        };
                    draw_chip(ui, "quiet", quiet_count, egui::Color32::from_gray(180));
                    draw_chip(
                        ui,
                        "active",
                        active_count,
                        egui::Color32::from_rgb(90, 200, 120),
                    );
                    draw_chip(
                        ui,
                        "alert",
                        alert_count,
                        egui::Color32::from_rgb(255, 120, 120),
                    );
                });
                ui.add_space(8.0);

                egui::CollapsingHeader::new("Lane Drilldown")
                    .default_open(true)
                    .show(ui, |ui| {
                        for lane in lane_summaries {
                            let color = match lane.signal {
                                AnalyzerSignal::Quiet => egui::Color32::from_gray(180),
                                AnalyzerSignal::Active => egui::Color32::from_rgb(90, 200, 120),
                                AnalyzerSignal::Alert => egui::Color32::from_rgb(255, 120, 120),
                            };
                            let signal_label = match lane.signal {
                                AnalyzerSignal::Quiet => "quiet",
                                AnalyzerSignal::Active => "active",
                                AnalyzerSignal::Alert => "alert",
                            };
                            egui::Frame::new()
                                .fill(color.gamma_multiply(0.08))
                                .stroke(egui::Stroke::new(1.0, color))
                                .corner_radius(8.0)
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong(lane.label);
                                        ui.separator();
                                        ui.monospace(lane.analyzer_id);
                                        ui.separator();
                                        ui.colored_label(color, signal_label);
                                    });
                                    ui.small(&lane.summary);
                                    if let Some(remediation) =
                                        Self::remediation_hint_for_analyzer(lane.analyzer_id)
                                    {
                                        ui.small(format!("remediation: {remediation}"));
                                    }
                                    egui::Grid::new(format!(
                                        "diag_lane_drilldown_{}",
                                        lane.lane_id
                                    ))
                                    .num_columns(2)
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.strong("Channel");
                                        ui.strong("Count");
                                        ui.end_row();
                                        for (channel_id, count) in lane.channel_counts {
                                            ui.monospace(channel_id);
                                            if count > 0 {
                                                ui.colored_label(color, count.to_string());
                                            } else {
                                                ui.monospace("0");
                                            }
                                            ui.end_row();
                                        }
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    });

                egui::CollapsingHeader::new("Channel Trends")
                    .default_open(true)
                    .show(ui, |ui| {
                        if channel_trends.is_empty() {
                            ui.small("No channel trend samples yet.");
                        } else {
                            egui::Grid::new("diag_channel_trends")
                                .num_columns(6)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Pin");
                                    ui.strong("Channel");
                                    ui.strong("Count");
                                    ui.strong("Avg");
                                    ui.strong("Trend");
                                    ui.strong("Recent samples");
                                    ui.end_row();

                                    for trend in channel_trends {
                                        let pin_label =
                                            if self.pinned_channels.contains(trend.channel_id) {
                                                "Unpin"
                                            } else {
                                                "Pin"
                                            };
                                        if ui.small_button(pin_label).clicked() {
                                            if !self.pinned_channels.insert(trend.channel_id) {
                                                self.pinned_channels.remove(trend.channel_id);
                                            }
                                        }
                                        if ui
                                            .selectable_label(
                                                self.selected_analysis_channel
                                                    == Some(trend.channel_id),
                                                trend.channel_id,
                                            )
                                            .clicked()
                                        {
                                            self.selected_analysis_channel = Some(trend.channel_id);
                                        }
                                        ui.monospace(trend.message_count.to_string());
                                        ui.monospace(format!(
                                            "{:.1}ms",
                                            trend.avg_latency_us as f64 / 1000.0
                                        ));
                                        let trend_color = match trend.trend {
                                            "rising" => egui::Color32::from_rgb(255, 120, 120),
                                            "falling" => egui::Color32::from_rgb(120, 180, 255),
                                            _ => egui::Color32::from_gray(180),
                                        };
                                        ui.colored_label(trend_color, trend.trend);
                                        let sample_preview = if trend.recent_samples_us.is_empty() {
                                            "-".to_string()
                                        } else {
                                            trend
                                                .recent_samples_us
                                                .iter()
                                                .map(|sample| {
                                                    format!("{:.1}", *sample as f64 / 1000.0)
                                                })
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        };
                                        ui.monospace(sample_preview);
                                        ui.end_row();
                                    }
                                });
                        }
                    });

                egui::CollapsingHeader::new("Channel History")
                    .default_open(false)
                    .show(ui, |ui| {
                        if channel_histories.is_empty() {
                            ui.small("No bucketed channel history yet.");
                        } else {
                            egui::Grid::new("diag_channel_history")
                                .num_columns(3)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Channel");
                                    ui.strong("Count buckets");
                                    ui.strong("Latency buckets");
                                    ui.end_row();

                                    for history in channel_histories {
                                        ui.monospace(history.channel_id);
                                        ui.monospace(
                                            history
                                                .count_buckets
                                                .iter()
                                                .map(|count| count.to_string())
                                                .collect::<Vec<_>>()
                                                .join(", "),
                                        );
                                        ui.monospace(
                                            history
                                                .latency_buckets_us
                                                .iter()
                                                .map(|sample| {
                                                    format!("{:.1}", *sample as f64 / 1000.0)
                                                })
                                                .collect::<Vec<_>>()
                                                .join(", "),
                                        );
                                        ui.end_row();
                                    }
                                });
                        }
                    });

                egui::CollapsingHeader::new("Channel Receipts")
                    .default_open(true)
                    .show(ui, |ui| {
                        if let Some(channel_id) = self.selected_analysis_channel {
                            let receipts = self.recent_channel_receipts(channel_id, 12);
                            ui.small(format!("Recent receipts for {channel_id}"));
                            if receipts.is_empty() {
                                ui.small(
                                    "No matching send/receive events in the current event ring.",
                                );
                            } else {
                                egui::Grid::new("diag_channel_receipts")
                                    .num_columns(4)
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.strong("Channel");
                                        ui.strong("Direction");
                                        ui.strong("Detail");
                                        ui.strong("Payload");
                                        ui.end_row();

                                        for receipt in receipts {
                                            ui.monospace(receipt.channel_id);
                                            ui.monospace(receipt.direction);
                                            ui.monospace(receipt.detail);
                                            let payload_summary = if receipt.payload_fields.is_empty() {
                                                "-".to_string()
                                            } else {
                                                receipt
                                                    .payload_fields
                                                    .iter()
                                                    .map(|field| format!("{}={}", field.name, field.value))
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            };
                                            ui.monospace(payload_summary);
                                            ui.end_row();
                                        }
                                    });
                            }
                        } else {
                            ui.small(
                                "Select a channel in Channel Trends to inspect recent receipts.",
                            );
                        }
                    });

                egui::CollapsingHeader::new("Signal Trace")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.small(format!(
                            "Last {} signals (ring capacity 128)",
                            signal_trace.len(),
                        ));
                        if signal_trace.is_empty() {
                            ui.small("No signals recorded yet.");
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("diag_signal_trace_scroll")
                                .max_height(220.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("diag_signal_trace")
                                        .num_columns(5)
                                        .striped(true)
                                        .show(ui, |ui| {
                                            ui.strong("Kind");
                                            ui.strong("Topic");
                                            ui.strong("Source");
                                            ui.strong("Notified");
                                            ui.strong("Failures");
                                            ui.end_row();

                                            for entry in signal_trace.iter().rev() {
                                                let (kind_label, topic_label) =
                                                    signal_trace_labels(&entry.kind);
                                                let failure_color = if entry.observer_failures > 0 {
                                                    egui::Color32::from_rgb(255, 120, 120)
                                                } else {
                                                    ui.visuals().text_color()
                                                };
                                                let unrouted_color =
                                                    if entry.observers_notified == 0 {
                                                        egui::Color32::from_rgb(200, 160, 60)
                                                    } else {
                                                        ui.visuals().text_color()
                                                    };
                                                ui.monospace(kind_label);
                                                ui.monospace(topic_label);
                                                ui.monospace(format!("{:?}", entry.source));
                                                ui.colored_label(
                                                    unrouted_color,
                                                    entry.observers_notified.to_string(),
                                                );
                                                ui.colored_label(
                                                    failure_color,
                                                    entry.observer_failures.to_string(),
                                                );
                                                ui.end_row();
                                            }
                                        });
                                });
                        }
                    });

                egui::CollapsingHeader::new("Analyzer Registry")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("diag_analysis_analyzers")
                            .num_columns(6)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Label");
                                ui.strong("Pin");
                                ui.strong("Analyzer");
                                ui.strong("Signal");
                                ui.strong("Runs");
                                ui.strong("Summary");
                                ui.end_row();

                                for analyzer in analyzer_snapshots {
                                    ui.label(analyzer.label);
                                    let pin_label =
                                        if self.pinned_analyzer_ids.contains(analyzer.id) {
                                            "Unpin"
                                        } else {
                                            "Pin"
                                        };
                                    if ui.small_button(pin_label).clicked() {
                                        if !self.pinned_analyzer_ids.insert(analyzer.id) {
                                            self.pinned_analyzer_ids.remove(analyzer.id);
                                        }
                                    }
                                    ui.monospace(analyzer.id);
                                    let (signal_label, signal_color) = match analyzer
                                        .last_result
                                        .as_ref()
                                        .map(|result| result.signal)
                                        .unwrap_or(AnalyzerSignal::Quiet)
                                    {
                                        AnalyzerSignal::Quiet => {
                                            ("quiet", egui::Color32::from_gray(180))
                                        }
                                        AnalyzerSignal::Active => {
                                            ("active", egui::Color32::from_rgb(90, 200, 120))
                                        }
                                        AnalyzerSignal::Alert => {
                                            ("alert", egui::Color32::from_rgb(255, 120, 120))
                                        }
                                    };
                                    ui.colored_label(signal_color, signal_label);
                                    ui.monospace(analyzer.run_count.to_string());
                                    if let Some(result) = analyzer.last_result {
                                        let mut summary = result.summary;
                                        if let Some(remediation) =
                                            Self::remediation_hint_for_analyzer(analyzer.id)
                                        {
                                            summary =
                                                format!("{summary} | remediation: {remediation}");
                                        }
                                        ui.label(summary);
                                    } else {
                                        ui.label("not yet run");
                                    }
                                    ui.end_row();
                                }
                            });
                    });

                egui::CollapsingHeader::new("Lane Harness")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_test_harness_scaffold(ui);
                    });
            }
            DiagnosticsTab::Compositor => {
                let replay_summary = self.compositor_replay_summary();
                ui.horizontal(|ui| {
                    ui.monospace(format!("history={}", self.compositor_state.frames.len()));
                    ui.separator();
                    ui.monospace(format!(
                        "replay_samples={}",
                        replay_summary["sample_count"].as_u64().unwrap_or(0)
                    ));
                    ui.separator();
                    ui.monospace(format!(
                        "replay_violations={}",
                        replay_summary["violation_count"].as_u64().unwrap_or(0)
                    ));
                    ui.separator();
                    match self.pinned_node_key {
                        Some(node_key) => {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 120, 120),
                                format!("pin={:?}", node_key),
                            );
                            if ui.button("Clear pin").clicked() {
                                self.pinned_node_key = None;
                            }
                        }
                        None => {
                            ui.small("pin=none");
                        }
                    }
                });
                ui.separator();
                ui.label("Compositor replay summary");
                egui::Grid::new("diagnostics_compositor_replay")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Metric");
                        ui.strong("Value");
                        ui.end_row();

                        ui.monospace("sample_count");
                        ui.monospace(replay_summary["sample_count"].to_string());
                        ui.end_row();

                        ui.monospace("violation_count");
                        ui.monospace(replay_summary["violation_count"].to_string());
                        ui.end_row();

                        ui.monospace("latest_sequence");
                        ui.monospace(replay_summary["latest_sequence"].to_string());
                        ui.end_row();

                        ui.monospace("latest_violation_node");
                        ui.monospace(replay_summary["latest_violation_node"].to_string());
                        ui.end_row();

                        ui.monospace("latest_duration_us");
                        ui.monospace(replay_summary["latest_duration_us"].to_string());
                        ui.end_row();

                        ui.monospace("bridge_probe_count");
                        ui.monospace(replay_summary["bridge_probe_count"].to_string());
                        ui.end_row();

                        ui.monospace("bridge_failed_frame_count");
                        ui.monospace(replay_summary["bridge_failed_frame_count"].to_string());
                        ui.end_row();

                        ui.monospace("avg_bridge_callback_us");
                        ui.monospace(replay_summary["avg_bridge_callback_us"].to_string());
                        ui.end_row();

                        ui.monospace("avg_bridge_presentation_us");
                        ui.monospace(replay_summary["avg_bridge_presentation_us"].to_string());
                        ui.end_row();
                    });
                ui.small(
                    "Save Snapshot JSON includes compositor replay artifacts and path details.",
                );
                ui.separator();

                let Some(last) = self.compositor_state.frames.back() else {
                    ui.small("No compositor frame samples yet.");
                    return;
                };

                ui.horizontal(|ui| {
                    ui.monospace(format!("seq={}", last.sequence));
                    ui.separator();
                    ui.monospace(format!("active_tiles={}", last.active_tile_count));
                    ui.separator();
                    ui.monospace(format!("focused_node={}", last.focused_node_present));
                });
                ui.separator();
                let differential = self.compositor_differential_summary();
                ui.label("Differential composition summary");
                egui::Grid::new("diagnostics_compositor_differential")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Metric");
                        ui.strong("Value");
                        ui.end_row();

                        ui.monospace("content_composed_count");
                        ui.monospace(differential["content_composed_count"].to_string());
                        ui.end_row();
                        ui.monospace("content_skipped_count");
                        ui.monospace(differential["content_skipped_count"].to_string());
                        ui.end_row();
                        ui.monospace("fallback_no_prior_signature_count");
                        ui.monospace(differential["fallback_no_prior_signature_count"].to_string());
                        ui.end_row();
                        ui.monospace("fallback_signature_changed_count");
                        ui.monospace(differential["fallback_signature_changed_count"].to_string());
                        ui.end_row();
                        ui.monospace("computed_skip_rate_basis_points");
                        ui.monospace(differential["computed_skip_rate_basis_points"].to_string());
                        ui.end_row();
                        ui.monospace("content_culled_offviewport_count");
                        ui.monospace(differential["content_culled_offviewport_count"].to_string());
                        ui.end_row();
                        ui.monospace("degradation_gpu_pressure_count");
                        ui.monospace(differential["degradation_gpu_pressure_count"].to_string());
                        ui.end_row();
                        ui.monospace("degradation_placeholder_mode_count");
                        ui.monospace(
                            differential["degradation_placeholder_mode_count"].to_string(),
                        );
                        ui.end_row();
                        ui.monospace("resource_reuse_context_hit_count");
                        ui.monospace(differential["resource_reuse_context_hit_count"].to_string());
                        ui.end_row();
                        ui.monospace("resource_reuse_context_miss_count");
                        ui.monospace(differential["resource_reuse_context_miss_count"].to_string());
                        ui.end_row();
                        ui.monospace("overlay_batch_sample_count");
                        ui.monospace(differential["overlay_batch_sample_count"].to_string());
                        ui.end_row();
                        ui.monospace("avg_skip_rate_basis_points");
                        ui.monospace(differential["avg_skip_rate_basis_points"].to_string());
                        ui.end_row();
                        ui.monospace("avg_overlay_batch_size");
                        ui.monospace(differential["avg_overlay_batch_size"].to_string());
                        ui.end_row();
                    });
                ui.separator();
                ui.label("Active tile hierarchy");
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for item in &last.hierarchy {
                                let selected = self.pinned_node_key == item.node_key
                                    && item.node_key.is_some();
                                let resp = ui.selectable_label(selected, &item.line);
                                if resp.clicked() {
                                    if self.pinned_node_key == item.node_key {
                                        self.pinned_node_key = None;
                                    } else {
                                        self.pinned_node_key = item.node_key;
                                    }
                                    if let Some(node_key) = item.node_key {
                                        self.pending_focus_node = Some(node_key);
                                    }
                                }
                            }
                        });
                });
                ui.separator();
                ui.label("Minimap (tiles vs viewport)");
                let minimap_size = egui::vec2(240.0, 140.0);
                let (minimap_rect, _) = ui.allocate_exact_size(minimap_size, egui::Sense::hover());
                let painter = ui.painter_at(minimap_rect);
                painter.rect_stroke(
                    minimap_rect,
                    3.0,
                    egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
                    egui::StrokeKind::Inside,
                );
                let content_rect = last.content_rect;
                let content_w = content_rect.width().max(1.0);
                let content_h = content_rect.height().max(1.0);
                let map_rect = |source: egui::Rect| {
                    let rel_min_x =
                        ((source.min.x - content_rect.min.x) / content_w).clamp(0.0, 1.0);
                    let rel_max_x =
                        ((source.max.x - content_rect.min.x) / content_w).clamp(0.0, 1.0);
                    let rel_min_y =
                        ((source.min.y - content_rect.min.y) / content_h).clamp(0.0, 1.0);
                    let rel_max_y =
                        ((source.max.y - content_rect.min.y) / content_h).clamp(0.0, 1.0);
                    egui::Rect::from_min_max(
                        egui::pos2(
                            minimap_rect.left() + minimap_rect.width() * rel_min_x,
                            minimap_rect.top() + minimap_rect.height() * rel_min_y,
                        ),
                        egui::pos2(
                            minimap_rect.left() + minimap_rect.width() * rel_max_x,
                            minimap_rect.top() + minimap_rect.height() * rel_max_y,
                        ),
                    )
                };

                for visible_rect in last.visible_regions.as_slice() {
                    painter.rect_filled(
                        map_rect(*visible_rect),
                        2.0,
                        egui::Color32::from_rgba_unmultiplied(110, 190, 255, 32),
                    );
                }
                for occlusion_rect in &last.occluding_host_rects {
                    let mapped_rect = map_rect(*occlusion_rect);
                    painter.rect_filled(
                        mapped_rect,
                        2.0,
                        egui::Color32::from_rgba_unmultiplied(255, 215, 0, 56),
                    );
                    painter.rect_stroke(
                        mapped_rect,
                        2.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 215, 0)),
                        egui::StrokeKind::Inside,
                    );
                }
                for tile in &last.tiles {
                    let r = map_rect(tile.rect);
                    painter.rect_stroke(
                        r,
                        2.0,
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 120, 120)),
                        egui::StrokeKind::Inside,
                    );
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("diagnostics_compositor_grid")
                        .num_columns(8)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Node");
                            ui.strong("URL");
                            ui.strong("Mapped");
                            ui.strong("Context");
                            ui.strong("PaintCb");
                            ui.strong("Path");
                            ui.strong("Rect");
                            ui.strong("W");
                            ui.strong("H");
                            ui.end_row();

                            for tile in &last.tiles {
                                let url = graph_app
                                    .workspace
                                    .domain
                                    .graph
                                    .get_node(tile.node_key)
                                    .map(|n| n.url().to_string())
                                    .unwrap_or_else(|| "<missing>".to_string());
                                let selected = self.pinned_node_key == Some(tile.node_key);
                                let hover =
                                    ui.selectable_label(selected, format!("{:?}", tile.node_key));
                                if hover.hovered() {
                                    self.hovered_node_key = Some(tile.node_key);
                                }
                                if hover.clicked() {
                                    if self.pinned_node_key == Some(tile.node_key) {
                                        self.pinned_node_key = None;
                                    } else {
                                        self.pinned_node_key = Some(tile.node_key);
                                    }
                                    self.pending_focus_node = Some(tile.node_key);
                                }
                                ui.label(url);
                                ui.monospace(format!("{}", tile.mapped_webview));
                                ui.monospace(format!("{}", tile.has_context));
                                ui.monospace(format!("{}", tile.paint_callback_registered));
                                ui.monospace(tile.render_path_hint);
                                ui.monospace(format!(
                                    "[{:.0},{:.0}]..[{:.0},{:.0}]",
                                    tile.rect.min.x,
                                    tile.rect.min.y,
                                    tile.rect.max.x,
                                    tile.rect.max.y
                                ));
                                ui.monospace(format!("{:.0}", tile.rect.width()));
                                ui.monospace(format!("{:.0}", tile.rect.height()));
                                ui.end_row();
                            }
                        });
                });
            }
            DiagnosticsTab::Intents => {
                ui.label("Recent GraphIntent stream");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in self.intents.iter().rev() {
                        ui.horizontal(|ui| {
                            if let Some(cause) = entry.cause {
                                let badge_color = match cause {
                                    LifecycleCause::UserSelect | LifecycleCause::Restore => {
                                        egui::Color32::from_rgb(80, 170, 255)
                                    }
                                    LifecycleCause::ActiveTileVisible
                                    | LifecycleCause::SelectedPrewarm => {
                                        egui::Color32::from_rgb(90, 200, 120)
                                    }
                                    LifecycleCause::ActiveLruEviction
                                    | LifecycleCause::WarmLruEviction
                                    | LifecycleCause::WorkspaceRetention => {
                                        egui::Color32::from_rgb(220, 170, 90)
                                    }
                                    LifecycleCause::Crash
                                    | LifecycleCause::MemoryPressureWarning
                                    | LifecycleCause::MemoryPressureCritical
                                    | LifecycleCause::CreateRetryExhausted => {
                                        egui::Color32::from_rgb(230, 100, 100)
                                    }
                                    LifecycleCause::ExplicitClose | LifecycleCause::NodeRemoval => {
                                        egui::Color32::from_rgb(180, 140, 220)
                                    }
                                };
                                ui.colored_label(badge_color, format!("[{:?}]", cause));
                            }
                            ui.monospace(&entry.line);
                        });
                    }
                });
            }
        }
    }
}

fn signal_trace_labels(
    kind: &crate::shell::desktop::runtime::registries::signal_routing::SignalKind,
) -> (&'static str, &'static str) {
    use crate::shell::desktop::runtime::registries::signal_routing::{
        InputEventSignal, LifecycleSignal, NavigationSignal, RegistryEventSignal, SignalKind,
        SyncSignal,
    };
    match kind {
        SignalKind::Navigation(nav) => {
            let label = match nav {
                NavigationSignal::Resolved { .. } => "Resolved",
                NavigationSignal::NodeActivated { .. } => "NodeActivated",
                NavigationSignal::MimeResolved { .. } => "MimeResolved",
            };
            (label, "Navigation")
        }
        SignalKind::Lifecycle(lc) => {
            let label = match lc {
                LifecycleSignal::SemanticIndexUpdated { .. } => "SemanticIndexUpdated",
                LifecycleSignal::MimeResolved { .. } => "MimeResolved",
                LifecycleSignal::WorkflowActivated { .. } => "WorkflowActivated",
                LifecycleSignal::MemoryPressureChanged { .. } => "MemoryPressureChanged",
                LifecycleSignal::UserIdle { .. } => "UserIdle",
                LifecycleSignal::UserResumed => "UserResumed",
            };
            (label, "Lifecycle")
        }
        SignalKind::Sync(sync) => {
            let label = match sync {
                SyncSignal::RemoteEntriesQueued => "RemoteEntriesQueued",
            };
            (label, "Sync")
        }
        SignalKind::RegistryEvent(re) => {
            let label = match re {
                RegistryEventSignal::ThemeChanged { .. } => "ThemeChanged",
                RegistryEventSignal::LensChanged { .. } => "LensChanged",
                RegistryEventSignal::WorkflowChanged { .. } => "WorkflowChanged",
                RegistryEventSignal::PhysicsProfileChanged { .. } => "PhysicsProfileChanged",
                RegistryEventSignal::CanvasProfileChanged { .. } => "CanvasProfileChanged",
                RegistryEventSignal::WorkbenchSurfaceChanged { .. } => "WorkbenchSurfaceChanged",
                RegistryEventSignal::SemanticIndexUpdated { .. } => "SemanticIndexUpdated",
                RegistryEventSignal::SettingsRouteRequested { .. } => "SettingsRouteRequested",
                RegistryEventSignal::ModLoaded { .. } => "ModLoaded",
                RegistryEventSignal::ModUnloaded { .. } => "ModUnloaded",
                RegistryEventSignal::AgentSpawned { .. } => "AgentSpawned",
                RegistryEventSignal::IdentityRotated { .. } => "IdentityRotated",
                RegistryEventSignal::WorkbenchProjectionRefreshRequested { .. } => {
                    "WorkbenchProjectionRefreshRequested"
                }
            };
            (label, "RegistryEvent")
        }
        SignalKind::InputEvent(ie) => {
            let label = match ie {
                InputEventSignal::ContextChanged { .. } => "ContextChanged",
                InputEventSignal::BindingRemapped { .. } => "BindingRemapped",
                InputEventSignal::BindingsReset => "BindingsReset",
            };
            (label, "InputEvent")
        }
    }
}
