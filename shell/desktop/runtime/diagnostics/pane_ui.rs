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
    ) {
        self.sync_history_health_snapshot_from_app(graph_app);
        self.sync_runtime_cache_snapshot_from_app(graph_app);
        self.sync_tracing_perf_snapshot_from_runtime();
        self.tick_drain();
        self.hovered_node_key = None;

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, DiagnosticsTab::Engine, "Engine");
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
                    "history_health: preview_active={} failures={}",
                    history_preview, history_failures
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
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::Grid::new("diag_active_analyzers")
                                .num_columns(4)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Analyzer");
                                    ui.strong("Signal");
                                    ui.strong("Runs");
                                    ui.strong("Summary");
                                    ui.end_row();

                                    for analyzer in analyzer_snapshots {
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
                                            ui.label(result.summary);
                                        } else {
                                            ui.label("not yet run");
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                }
                self.render_test_harness_scaffold(ui);
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
                let viewport = last.viewport_rect;
                let viewport_w = viewport.width().max(1.0);
                let viewport_h = viewport.height().max(1.0);
                for tile in &last.tiles {
                    let rel_min_x =
                        ((tile.rect.min.x - viewport.min.x) / viewport_w).clamp(0.0, 1.0);
                    let rel_max_x =
                        ((tile.rect.max.x - viewport.min.x) / viewport_w).clamp(0.0, 1.0);
                    let rel_min_y =
                        ((tile.rect.min.y - viewport.min.y) / viewport_h).clamp(0.0, 1.0);
                    let rel_max_y =
                        ((tile.rect.max.y - viewport.min.y) / viewport_h).clamp(0.0, 1.0);
                    let r = egui::Rect::from_min_max(
                        egui::pos2(
                            minimap_rect.left() + minimap_rect.width() * rel_min_x,
                            minimap_rect.top() + minimap_rect.height() * rel_min_y,
                        ),
                        egui::pos2(
                            minimap_rect.left() + minimap_rect.width() * rel_max_x,
                            minimap_rect.top() + minimap_rect.height() * rel_max_y,
                        ),
                    );
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
                                    .map(|n| n.url.clone())
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
