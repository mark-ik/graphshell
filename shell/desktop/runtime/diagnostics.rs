/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::SystemTime;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause};
use crate::graph::NodeKey;
use crate::registries::atomic::diagnostics as diagnostics_registry;
use crate::services::persistence::GraphStore;
use crate::shell::desktop::workbench::compositor_adapter::{
    CompositorReplaySample, replay_samples_snapshot,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT,
    CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
    CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE,
    CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI, CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER, CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
    CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT,
    CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
    CHANNEL_DIAGNOSTICS_CONFIG_CHANGED, CHANNEL_INVARIANT_TIMEOUT,
};

static GLOBAL_DIAGNOSTICS_TX: OnceLock<Sender<DiagnosticEvent>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_DIAGNOSTICS_TX: std::cell::RefCell<Option<Sender<DiagnosticEvent>>> =
        std::cell::RefCell::new(None);
}

pub(crate) fn install_global_sender(sender: Sender<DiagnosticEvent>) {
    let _ = GLOBAL_DIAGNOSTICS_TX.set(sender.clone());

    #[cfg(test)]
    {
        TEST_DIAGNOSTICS_TX.with(|slot| {
            *slot.borrow_mut() = Some(sender.clone());
        });
    }
}

pub(crate) fn emit_event(event: DiagnosticEvent) {
    #[cfg(test)]
    {
        let mut event = Some(event);
        let mut handled = false;
        TEST_DIAGNOSTICS_TX.with(|slot| {
            if let Some(tx) = slot.borrow().as_ref() {
                if let Some(payload) = event.take() {
                    emit_event_with_sender(tx, payload);
                }
                handled = true;
            }
        });
        if handled {
            return;
        }
        if let Some(tx) = GLOBAL_DIAGNOSTICS_TX.get() {
            if let Some(payload) = event.take() {
                emit_event_with_sender(tx, payload);
            }
        }
    }

    #[cfg(not(test))]
    {
        if let Some(tx) = GLOBAL_DIAGNOSTICS_TX.get() {
            emit_event_with_sender(tx, event);
        }
    }
}

fn emit_event_with_sender(tx: &Sender<DiagnosticEvent>, event: DiagnosticEvent) {
    let mut derived_events = Vec::new();
    let mut should_emit = true;

    match &event {
        DiagnosticEvent::MessageSent { channel_id, .. }
        | DiagnosticEvent::MessageReceived { channel_id, .. } => {
            let (allowed, violations) = diagnostics_registry::should_emit_and_observe(channel_id);
            should_emit = allowed;
            for violation in violations {
                derived_events.push(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_INVARIANT_TIMEOUT,
                    byte_len: violation
                        .invariant_id
                        .len()
                        .saturating_add(violation.start_channel.len()),
                });
            }
        }
        _ => {}
    }

    if should_emit {
        let _ = tx.send(event);
    }

    for derived in derived_events {
        let _ = tx.send(derived);
    }
}

pub(crate) fn emit_span_duration(name: &'static str, duration_us: u64) {
    emit_event(DiagnosticEvent::Span {
        name,
        phase: SpanPhase::Exit,
        duration_us: Some(duration_us),
    });
}

fn config_changed_payload_len(
    channel_id: &str,
    config: &diagnostics_registry::ChannelConfig,
) -> usize {
    format!(
        "{}|{}|{:.2}|{}",
        channel_id, config.enabled, config.sample_rate, config.retention_count
    )
    .len()
}

fn config_changed_event(
    channel_id: &str,
    config: &diagnostics_registry::ChannelConfig,
) -> DiagnosticEvent {
    DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_DIAGNOSTICS_CONFIG_CHANGED,
        byte_len: config_changed_payload_len(channel_id, config),
    }
}

fn emit_channel_config_changed(channel_id: &str, config: &diagnostics_registry::ChannelConfig) {
    emit_event(config_changed_event(channel_id, config));
}

#[allow(dead_code)]
pub(crate) fn apply_channel_config_update(
    graph_app: &mut GraphBrowserApp,
    channel_id: &str,
    config: diagnostics_registry::ChannelConfig,
) {
    diagnostics_registry::set_channel_config_global(channel_id, config.clone());
    graph_app.set_diagnostics_channel_config(channel_id, &config);
    emit_channel_config_changed(channel_id, &config);
}

pub(crate) fn apply_channel_config_update_with_diagnostics(
    diagnostics_state: &DiagnosticsState,
    graph_app: &mut GraphBrowserApp,
    channel_id: &str,
    config: diagnostics_registry::ChannelConfig,
) {
    diagnostics_registry::set_channel_config_global(channel_id, config.clone());
    graph_app.set_diagnostics_channel_config(channel_id, &config);
    let _ = diagnostics_state
        .event_tx
        .send(config_changed_event(channel_id, &config));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticsTab {
    Engine,
    Compositor,
    Intents,
}

#[derive(Clone, Debug)]
pub(crate) struct CompositorTileSample {
    pub(crate) node_key: NodeKey,
    pub(crate) rect: egui::Rect,
    pub(crate) mapped_webview: bool,
    pub(crate) has_context: bool,
    pub(crate) paint_callback_registered: bool,
    pub(crate) render_path_hint: &'static str,
}

#[derive(Clone, Debug)]
pub(crate) struct CompositorFrameSample {
    pub(crate) sequence: u64,
    pub(crate) active_tile_count: usize,
    pub(crate) focused_node_present: bool,
    pub(crate) viewport_rect: egui::Rect,
    pub(crate) hierarchy: Vec<HierarchySample>,
    pub(crate) tiles: Vec<CompositorTileSample>,
}

#[derive(Clone, Debug)]
pub(crate) struct HierarchySample {
    pub(crate) line: String,
    pub(crate) node_key: Option<NodeKey>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpanPhase {
    Enter,
    Exit,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum DiagnosticEvent {
    Span {
        name: &'static str,
        phase: SpanPhase,
        duration_us: Option<u64>,
    },
    MessageSent {
        channel_id: &'static str,
        byte_len: usize,
    },
    MessageReceived {
        channel_id: &'static str,
        latency_us: u64,
    },
    CompositorFrame(CompositorFrameSample),
    IntentBatch(Vec<GraphIntent>),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CompositorState {
    pub(crate) frames: VecDeque<CompositorFrameSample>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DiagnosticGraph {
    pub(crate) message_counts: HashMap<&'static str, u64>,
    pub(crate) message_bytes_sent: HashMap<&'static str, u64>,
    pub(crate) message_latency_us: HashMap<&'static str, u64>,
    pub(crate) message_latency_samples: HashMap<&'static str, u64>,
    pub(crate) message_latency_recent_us: HashMap<&'static str, VecDeque<u64>>,
    pub(crate) span_enter_counts: HashMap<&'static str, u64>,
    pub(crate) span_exit_counts: HashMap<&'static str, u64>,
    pub(crate) last_span_duration_us: HashMap<&'static str, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnalyzerSignal {
    Quiet,
    Active,
    Alert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnalyzerResult {
    pub(crate) signal: AnalyzerSignal,
    pub(crate) summary: String,
}

type DiagnosticsAnalyzerFn = fn(&DiagnosticGraph, &VecDeque<DiagnosticEvent>) -> AnalyzerResult;

#[derive(Clone, Debug)]
struct RegisteredAnalyzer {
    id: &'static str,
    label: &'static str,
    analyze: DiagnosticsAnalyzerFn,
    run_count: u64,
    last_result: Option<AnalyzerResult>,
}

#[derive(Clone, Debug)]
pub(crate) struct AnalyzerSnapshot {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) run_count: u64,
    pub(crate) last_result: Option<AnalyzerResult>,
}

#[derive(Clone, Debug, Default)]
struct AnalyzerRegistry {
    analyzers: Vec<RegisteredAnalyzer>,
}

impl AnalyzerRegistry {
    fn register(
        &mut self,
        id: &'static str,
        label: &'static str,
        analyze: DiagnosticsAnalyzerFn,
    ) -> bool {
        if self.analyzers.iter().any(|entry| entry.id == id) {
            return false;
        }

        self.analyzers.push(RegisteredAnalyzer {
            id,
            label,
            analyze,
            run_count: 0,
            last_result: None,
        });
        true
    }

    fn run_all(&mut self, graph: &DiagnosticGraph, event_ring: &VecDeque<DiagnosticEvent>) {
        for analyzer in &mut self.analyzers {
            analyzer.run_count = analyzer.run_count.saturating_add(1);
            analyzer.last_result = Some((analyzer.analyze)(graph, event_ring));
        }
    }

    fn snapshots(&self) -> Vec<AnalyzerSnapshot> {
        self.analyzers
            .iter()
            .map(|entry| AnalyzerSnapshot {
                id: entry.id,
                label: entry.label,
                run_count: entry.run_count,
                last_result: entry.last_result.clone(),
            })
            .collect()
    }
}

fn analyze_event_ring_pressure(
    _graph: &DiagnosticGraph,
    event_ring: &VecDeque<DiagnosticEvent>,
) -> AnalyzerResult {
    let size = event_ring.len();
    let (signal, summary) = if size >= 480 {
        (
            AnalyzerSignal::Alert,
            format!("event ring near capacity ({size}/512)"),
        )
    } else if size > 0 {
        (
            AnalyzerSignal::Active,
            format!("event ring receiving traffic ({size} events buffered)"),
        )
    } else {
        (AnalyzerSignal::Quiet, "event ring idle".to_string())
    };

    AnalyzerResult { signal, summary }
}

#[derive(Clone, Copy)]
struct EdgeMetric {
    count: u64,
    percentile_latency_us: u64,
    bottleneck: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LatencyPercentile {
    P90,
    P95,
    P99,
}

impl LatencyPercentile {
    fn label(self) -> &'static str {
        match self {
            Self::P90 => "p90",
            Self::P95 => "p95",
            Self::P99 => "p99",
        }
    }

    fn fraction(self) -> f64 {
        match self {
            Self::P90 => 0.90,
            Self::P95 => 0.95,
            Self::P99 => 0.99,
        }
    }
}

const DEFAULT_BOTTLENECK_LATENCY_US: u64 = 50_000;
const LATENCY_SAMPLE_WINDOW: usize = 256;

const CHANNELS_SEMANTIC_TO_INTENTS: [&str; 15] = [
    "semantic.events_ingest",
    "semantic.intents_emitted",
    "semantic.intent.url_changed",
    "semantic.intent.history_changed",
    "semantic.intent.title_changed",
    "semantic.intent.create_new_webview",
    "semantic.intent.create_new_webview_unmapped",
    "semantic.intent.webview_crashed",
    "window.graph_event.url_changed",
    "window.graph_event.history_changed",
    "window.graph_event.title_changed",
    "window.graph_event.create_new_webview",
    "window.graph_event.webview_crashed",
    "window.graph_event.drain",
    "window.graph_event.drain_count",
];

const CHANNELS_SERVO_TO_SEMANTIC: [&str; 8] = [
    "servo.delegate.url_changed",
    "servo.delegate.history_changed",
    "servo.delegate.title_changed",
    "servo.delegate.create_new_webview",
    "servo.delegate.webview_crashed",
    "servo.graph_event.drain",
    "servo.graph_event.drain_count",
    "servo.event_loop.spin",
];

const CHANNELS_INTENTS_TO_RENDER_PASS: [&str; 1] = ["graph_intents.apply"];

const CHANNELS_RENDER_PASS_TO_COMPOSITOR: [&str; 1] = ["tile_compositor.paint"];

const CHANNELS_BACKPRESSURE_TO_INTENTS: [&str; 3] = [
    "webview_backpressure.create_attempt",
    "webview_backpressure.cooldown",
    "webview_backpressure.timeout",
];

const CHANNELS_INTENTS_TO_COMPOSITOR: [&str; 1] = ["tile_compositor.paint"];
const CHANNEL_ACTIVE_TILE_VIOLATION: &str = "tile_render_pass.active_tile_violation";
const CHANNELS_COMPOSITOR_OVERLAY_STYLE: [(&str, &str); 2] = [
    ("RectStroke", CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE),
    ("ChromeOnly", CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY),
];
const CHANNELS_COMPOSITOR_OVERLAY_MODE: [(&str, &str); 4] = [
    (
        "CompositedTexture",
        CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    ),
    ("NativeOverlay", CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY),
    ("EmbeddedEgui", CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI),
    ("Placeholder", CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER),
];

#[derive(Clone, Debug)]
struct IntentSample {
    line: String,
    cause: Option<LifecycleCause>,
}

pub(crate) struct DiagnosticsState {
    active_tab: DiagnosticsTab,
    event_tx: Sender<DiagnosticEvent>,
    event_rx: Receiver<DiagnosticEvent>,
    event_ring: VecDeque<DiagnosticEvent>,
    last_drain_at: Instant,
    drain_interval: Duration,
    compositor_state: CompositorState,
    diagnostic_graph: DiagnosticGraph,
    intents: VecDeque<IntentSample>,
    capacity: usize,
    next_sequence: u64,
    hovered_node_key: Option<NodeKey>,
    pinned_node_key: Option<NodeKey>,
    pending_focus_node: Option<NodeKey>,
    latency_percentile: LatencyPercentile,
    bottleneck_latency_us: u64,
    export_feedback: Option<String>,
    analyzer_registry: AnalyzerRegistry,
}

impl DiagnosticsState {
    fn register_builtin_analyzers(&mut self) {
        let _ = self.register_analyzer(
            "diagnostics.event_ring_pressure",
            "Event Ring Pressure",
            analyze_event_ring_pressure,
        );
    }

    pub(crate) fn register_analyzer(
        &mut self,
        id: &'static str,
        label: &'static str,
        analyze: DiagnosticsAnalyzerFn,
    ) -> bool {
        self.analyzer_registry.register(id, label, analyze)
    }

    fn analyzer_snapshots(&self) -> Vec<AnalyzerSnapshot> {
        self.analyzer_registry.snapshots()
    }

    fn compositor_replay_summary(&self) -> Value {
        let samples = replay_samples_snapshot();
        let sample_count = samples.len() as u64;
        let violation_count = samples.iter().filter(|sample| sample.violation).count() as u64;
        let latest_sequence = samples.last().map(|sample| sample.sequence);
        let latest_violation_node = samples
            .iter()
            .rev()
            .find(|sample| sample.violation)
            .map(|sample| format!("{:?}", sample.node_key));
        let latest_duration_us = samples.last().map(|sample| sample.duration_us);
        let bridge_probe_count = self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE);
        let bridge_failed_frame_count =
            self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME);
        let bridge_callback_sample_count =
            self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE);
        let bridge_callback_total_us = self
            .diagnostic_graph
            .message_bytes_sent
            .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE)
            .copied()
            .unwrap_or(0);
        let avg_bridge_callback_us = if bridge_callback_sample_count == 0 {
            0
        } else {
            bridge_callback_total_us / bridge_callback_sample_count
        };
        let bridge_presentation_sample_count =
            self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE);
        let bridge_presentation_total_us = self
            .diagnostic_graph
            .message_bytes_sent
            .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE)
            .copied()
            .unwrap_or(0);
        let avg_bridge_presentation_us = if bridge_presentation_sample_count == 0 {
            0
        } else {
            bridge_presentation_total_us / bridge_presentation_sample_count
        };

        json!({
            "sample_count": sample_count,
            "violation_count": violation_count,
            "latest_sequence": latest_sequence,
            "latest_violation_node": latest_violation_node,
            "latest_duration_us": latest_duration_us,
            "bridge_probe_count": bridge_probe_count,
            "bridge_failed_frame_count": bridge_failed_frame_count,
            "avg_bridge_callback_us": avg_bridge_callback_us,
            "avg_bridge_presentation_us": avg_bridge_presentation_us,
        })
    }

    fn replay_export_feedback(path: &Path, replay_samples: &[CompositorReplaySample]) -> String {
        let violation_count = replay_samples
            .iter()
            .filter(|sample| sample.violation)
            .count();
        format!(
            "Saved JSON: {} (replay samples: {}, violations: {})",
            path.display(),
            replay_samples.len(),
            violation_count
        )
    }

    fn bridge_spike_measurement_value_from_samples(samples: &[CompositorReplaySample]) -> Value {
        let sample_count = samples.len() as u64;
        let failed_frame_count = samples
            .iter()
            .filter(|sample| sample.violation)
            .count() as u64;
        let callback_total_us: u64 = samples.iter().map(|sample| sample.callback_us).sum();
        let presentation_total_us: u64 = samples.iter().map(|sample| sample.presentation_us).sum();
        let avg_callback_us = if sample_count == 0 {
            0
        } else {
            callback_total_us / sample_count
        };
        let avg_presentation_us = if sample_count == 0 {
            0
        } else {
            presentation_total_us / sample_count
        };
        let chaos_enabled_sample_count =
            samples.iter().filter(|sample| sample.chaos_enabled).count() as u64;
        let restore_verification_fail_count =
            samples.iter().filter(|sample| !sample.restore_verified).count() as u64;
        let mut failed_by_reason: HashMap<&'static str, u64> = HashMap::new();
        for sample in samples.iter().filter(|sample| sample.violation) {
            if sample.viewport_changed {
                *failed_by_reason.entry("viewport").or_insert(0) += 1;
            }
            if sample.scissor_changed {
                *failed_by_reason.entry("scissor").or_insert(0) += 1;
            }
            if sample.blend_changed {
                *failed_by_reason.entry("blend").or_insert(0) += 1;
            }
            if sample.active_texture_changed {
                *failed_by_reason.entry("active_texture").or_insert(0) += 1;
            }
            if sample.framebuffer_binding_changed {
                *failed_by_reason.entry("framebuffer_binding").or_insert(0) += 1;
            }
        }

        let mut bridge_path_counts: HashMap<&'static str, u64> = HashMap::new();
        let mut bridge_mode_counts: HashMap<&'static str, u64> = HashMap::new();
        for sample in samples {
            *bridge_path_counts.entry(sample.bridge_path).or_insert(0) += 1;
            *bridge_mode_counts.entry(sample.bridge_mode).or_insert(0) += 1;
        }

        let latest = samples.last().map(|sample| {
            json!({
                "bridge_path": sample.bridge_path,
                "bridge_mode": sample.bridge_mode,
                "tile_rect_px": {
                    "x": sample.tile_rect_px[0],
                    "y": sample.tile_rect_px[1],
                    "width": sample.tile_rect_px[2],
                    "height": sample.tile_rect_px[3],
                },
                "render_size_px": {
                    "width": sample.render_size_px[0],
                    "height": sample.render_size_px[1],
                },
                "callback_us": sample.callback_us,
                "presentation_us": sample.presentation_us,
                "duration_us": sample.duration_us,
                "failed_frame": sample.violation,
                "chaos_enabled": sample.chaos_enabled,
                "restore_verified": sample.restore_verified,
                "failure_flags": {
                    "viewport": sample.viewport_changed,
                    "scissor": sample.scissor_changed,
                    "blend": sample.blend_changed,
                    "active_texture": sample.active_texture_changed,
                    "framebuffer_binding": sample.framebuffer_binding_changed,
                },
            })
        });

        let samples_json: Vec<Value> = samples
            .iter()
            .map(|sample| {
                json!({
                    "sequence": sample.sequence,
                    "node_key": format!("{:?}", sample.node_key),
                    "bridge_path": sample.bridge_path,
                    "bridge_mode": sample.bridge_mode,
                    "tile_rect_px": {
                        "x": sample.tile_rect_px[0],
                        "y": sample.tile_rect_px[1],
                        "width": sample.tile_rect_px[2],
                        "height": sample.tile_rect_px[3],
                    },
                    "render_size_px": {
                        "width": sample.render_size_px[0],
                        "height": sample.render_size_px[1],
                    },
                    "callback_us": sample.callback_us,
                    "presentation_us": sample.presentation_us,
                    "duration_us": sample.duration_us,
                    "failed_frame": sample.violation,
                    "chaos_enabled": sample.chaos_enabled,
                    "restore_verified": sample.restore_verified,
                    "failure_flags": {
                        "viewport": sample.viewport_changed,
                        "scissor": sample.scissor_changed,
                        "blend": sample.blend_changed,
                        "active_texture": sample.active_texture_changed,
                        "framebuffer_binding": sample.framebuffer_binding_changed,
                    },
                })
            })
            .collect();

        json!({
            "version": 1,
            "generated_at_unix_secs": Self::export_timestamp_secs(),
            "measurement_contract": {
                "bridge_path_used": bridge_path_counts,
                "bridge_mode_used": bridge_mode_counts,
                "sample_count": sample_count,
                "failed_frame_count": failed_frame_count,
                "avg_callback_us": avg_callback_us,
                "avg_presentation_us": avg_presentation_us,
                "chaos_enabled_sample_count": chaos_enabled_sample_count,
                "restore_verification_fail_count": restore_verification_fail_count,
                "failed_by_reason": failed_by_reason,
                "latest": latest,
            },
            "samples": samples_json,
        })
    }

    fn bridge_spike_measurement_value(&self) -> Value {
        let samples = replay_samples_snapshot();
        Self::bridge_spike_measurement_value_from_samples(&samples)
    }

    fn compositor_differential_summary(&self) -> Value {
        let composed_count = self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED);
        let skipped_count = self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED);
        let fallback_no_prior_count =
            self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE);
        let fallback_signature_changed_count =
            self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED);
        let evaluated_count = composed_count.saturating_add(skipped_count);
        let computed_skip_rate_basis_points = if evaluated_count == 0 {
            0
        } else {
            ((skipped_count * 10_000) / evaluated_count) as u64
        };
        let skip_rate_sample_count =
            self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE);
        let content_culled_offviewport_count =
            self.channel_count(CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT);
        let degradation_gpu_pressure_count =
            self.channel_count(CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE);
        let degradation_placeholder_mode_count =
            self.channel_count(CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE);
        let resource_reuse_context_hit_count =
            self.channel_count(CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT);
        let resource_reuse_context_miss_count =
            self.channel_count(CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS);
        let overlay_batch_sample_count = self.channel_count(CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE);
        let avg_skip_rate_basis_points = if skip_rate_sample_count == 0 {
            0
        } else {
            self.diagnostic_graph
                .message_bytes_sent
                .get(CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE)
                .copied()
                .unwrap_or(0)
                / skip_rate_sample_count
        };
        let avg_overlay_batch_size = if overlay_batch_sample_count == 0 {
            0
        } else {
            self.diagnostic_graph
                .message_bytes_sent
                .get(CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE)
                .copied()
                .unwrap_or(0)
                / overlay_batch_sample_count
        };

        json!({
            "content_composed_count": composed_count,
            "content_skipped_count": skipped_count,
            "fallback_no_prior_signature_count": fallback_no_prior_count,
            "fallback_signature_changed_count": fallback_signature_changed_count,
            "skip_rate_sample_count": skip_rate_sample_count,
            "content_culled_offviewport_count": content_culled_offviewport_count,
            "degradation_gpu_pressure_count": degradation_gpu_pressure_count,
            "degradation_placeholder_mode_count": degradation_placeholder_mode_count,
            "resource_reuse_context_hit_count": resource_reuse_context_hit_count,
            "resource_reuse_context_miss_count": resource_reuse_context_miss_count,
            "overlay_batch_sample_count": overlay_batch_sample_count,
            "computed_skip_rate_basis_points": computed_skip_rate_basis_points,
            "avg_skip_rate_basis_points": avg_skip_rate_basis_points,
            "avg_overlay_batch_size": avg_overlay_batch_size,
        })
    }

    fn render_compositor_overlay_buckets(&self, ui: &mut egui::Ui) {
        ui.label("Compositor overlay buckets");

        egui::Grid::new("diag_overlay_style_buckets")
            .num_columns(2)
            .show(ui, |ui| {
                ui.strong("Style");
                ui.strong("Count");
                ui.end_row();

                for (label, channel_id) in CHANNELS_COMPOSITOR_OVERLAY_STYLE {
                    let count = self
                        .diagnostic_graph
                        .message_counts
                        .get(channel_id)
                        .copied()
                        .unwrap_or(0);
                    ui.monospace(label);
                    ui.monospace(format!("{count}"));
                    ui.end_row();
                }
            });

        ui.add_space(4.0);

        egui::Grid::new("diag_overlay_mode_buckets")
            .num_columns(2)
            .show(ui, |ui| {
                ui.strong("RenderMode");
                ui.strong("Count");
                ui.end_row();

                for (label, channel_id) in CHANNELS_COMPOSITOR_OVERLAY_MODE {
                    let count = self
                        .diagnostic_graph
                        .message_counts
                        .get(channel_id)
                        .copied()
                        .unwrap_or(0);
                    ui.monospace(label);
                    ui.monospace(format!("{count}"));
                    ui.end_row();
                }
            });
    }

    pub(crate) fn new() -> Self {
        let (event_tx, event_rx) = unbounded();
        install_global_sender(event_tx.clone());
        let mut state = Self {
            active_tab: DiagnosticsTab::Compositor,
            event_tx,
            event_rx,
            event_ring: VecDeque::new(),
            last_drain_at: Instant::now(),
            drain_interval: Duration::from_millis(100),
            compositor_state: CompositorState::default(),
            diagnostic_graph: DiagnosticGraph::default(),
            intents: VecDeque::new(),
            capacity: 120,
            next_sequence: 1,
            hovered_node_key: None,
            pinned_node_key: None,
            pending_focus_node: None,
            latency_percentile: LatencyPercentile::P95,
            bottleneck_latency_us: DEFAULT_BOTTLENECK_LATENCY_US,
            export_feedback: None,
            analyzer_registry: AnalyzerRegistry::default(),
        };
        state.register_builtin_analyzers();
        state
    }

    pub(crate) fn push_frame(&mut self, sample: CompositorFrameSample) {
        let _ = self.event_tx.send(DiagnosticEvent::CompositorFrame(sample));
    }

    pub(crate) fn record_intents(&mut self, intents: &[GraphIntent]) {
        if intents.is_empty() {
            return;
        }
        let _ = self
            .event_tx
            .send(DiagnosticEvent::IntentBatch(intents.to_vec()));
    }

    pub(crate) fn record_span_duration(&self, name: &'static str, duration_us: u64) {
        let _ = self.event_tx.send(DiagnosticEvent::Span {
            name,
            phase: SpanPhase::Exit,
            duration_us: Some(duration_us),
        });
    }

    pub(crate) fn tick_drain(&mut self) {
        if self.last_drain_at.elapsed() < self.drain_interval {
            return;
        }
        self.last_drain_at = Instant::now();

        while let Ok(event) = self.event_rx.try_recv() {
            self.aggregate_event(&event);
            self.event_ring.push_back(event);
            while self.event_ring.len() > 512 {
                self.event_ring.pop_front();
            }
        }

        self.analyzer_registry
            .run_all(&self.diagnostic_graph, &self.event_ring);
    }

    fn aggregate_event(&mut self, event: &DiagnosticEvent) {
        match event {
            DiagnosticEvent::CompositorFrame(sample) => {
                let mut sample = sample.clone();
                sample.sequence = self.next_sequence;
                self.next_sequence = self.next_sequence.saturating_add(1);
                self.compositor_state.frames.push_back(sample);
                while self.compositor_state.frames.len() > self.capacity {
                    self.compositor_state.frames.pop_front();
                }
            }
            DiagnosticEvent::IntentBatch(intents) => {
                for intent in intents {
                    let cause = match intent {
                        GraphIntent::PromoteNodeToActive { cause, .. }
                        | GraphIntent::DemoteNodeToCold { cause, .. }
                        | GraphIntent::DemoteNodeToWarm { cause, .. }
                        | GraphIntent::ClearRuntimeBlocked { cause, .. } => Some(*cause),
                        _ => None,
                    };
                    self.intents.push_back(IntentSample {
                        line: format!("{:?}", intent),
                        cause,
                    });
                }
                while self.intents.len() > self.capacity {
                    self.intents.pop_front();
                }
            }
            DiagnosticEvent::Span {
                name,
                phase,
                duration_us: Some(us),
            } => {
                match phase {
                    SpanPhase::Enter => {
                        *self
                            .diagnostic_graph
                            .span_enter_counts
                            .entry(*name)
                            .or_insert(0) += 1;
                    }
                    SpanPhase::Exit => {
                        *self
                            .diagnostic_graph
                            .span_exit_counts
                            .entry(*name)
                            .or_insert(0) += 1;
                    }
                }
                self.diagnostic_graph
                    .last_span_duration_us
                    .insert(*name, *us);
            }
            DiagnosticEvent::Span { name, phase, .. } => match phase {
                SpanPhase::Enter => {
                    *self
                        .diagnostic_graph
                        .span_enter_counts
                        .entry(*name)
                        .or_insert(0) += 1;
                }
                SpanPhase::Exit => {
                    *self
                        .diagnostic_graph
                        .span_exit_counts
                        .entry(*name)
                        .or_insert(0) += 1;
                }
            },
            DiagnosticEvent::MessageSent {
                channel_id,
                byte_len,
            } => {
                *self
                    .diagnostic_graph
                    .message_counts
                    .entry(*channel_id)
                    .or_insert(0) += 1;
                *self
                    .diagnostic_graph
                    .message_bytes_sent
                    .entry(*channel_id)
                    .or_insert(0) += *byte_len as u64;
            }
            DiagnosticEvent::MessageReceived {
                channel_id,
                latency_us,
            } => {
                *self
                    .diagnostic_graph
                    .message_counts
                    .entry(*channel_id)
                    .or_insert(0) += 1;
                *self
                    .diagnostic_graph
                    .message_latency_us
                    .entry(*channel_id)
                    .or_insert(0) += *latency_us;
                *self
                    .diagnostic_graph
                    .message_latency_samples
                    .entry(*channel_id)
                    .or_insert(0) += 1;
                let samples = self
                    .diagnostic_graph
                    .message_latency_recent_us
                    .entry(*channel_id)
                    .or_default();
                samples.push_back(*latency_us);
                while samples.len() > LATENCY_SAMPLE_WINDOW {
                    samples.pop_front();
                }
            }
        }
    }

    fn percentile(values: &mut [u64], fraction: f64) -> u64 {
        if values.is_empty() {
            return 0;
        }
        values.sort_unstable();
        let rank = ((values.len() as f64) * fraction).ceil() as usize;
        let idx = rank.saturating_sub(1).min(values.len() - 1);
        values[idx]
    }

    #[cfg(test)]
    fn percentile_95(values: &mut [u64]) -> u64 {
        Self::percentile(values, 0.95)
    }

    fn selected_percentile_latency_us(&self, values: &mut [u64]) -> u64 {
        Self::percentile(values, self.latency_percentile.fraction())
    }

    fn edge_metric(&self, channels: &[&'static str]) -> EdgeMetric {
        let mut count = 0_u64;
        let mut recent_latencies = Vec::new();

        for channel in channels {
            count += self
                .diagnostic_graph
                .message_counts
                .get(channel)
                .copied()
                .unwrap_or(0);
            if let Some(samples) = self.diagnostic_graph.message_latency_recent_us.get(channel) {
                recent_latencies.extend(samples.iter().copied());
            }
        }

        let percentile_latency_us = self.selected_percentile_latency_us(&mut recent_latencies);
        EdgeMetric {
            count,
            percentile_latency_us,
            bottleneck: percentile_latency_us >= self.bottleneck_latency_us,
        }
    }

    fn clear_aggregates(&mut self) {
        self.event_ring.clear();
        self.compositor_state.frames.clear();
        self.diagnostic_graph = DiagnosticGraph::default();
        self.intents.clear();
    }

    fn channel_count(&self, channel: &'static str) -> u64 {
        self.diagnostic_graph
            .message_counts
            .get(channel)
            .copied()
            .unwrap_or(0)
    }

    fn export_dir() -> Result<PathBuf, String> {
        let dir = GraphStore::default_data_dir().join("diagnostics_exports");
        fs::create_dir_all(&dir).map_err(|e| {
            format!(
                "failed to create diagnostics export dir {}: {e}",
                dir.display()
            )
        })?;
        Ok(dir)
    }

    fn export_timestamp_secs() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn snapshot_json_value(&self) -> Value {
        let replay_samples: Vec<Value> = replay_samples_snapshot()
            .into_iter()
            .map(|sample| {
                json!({
                    "sequence": sample.sequence,
                    "node_key": format!("{:?}", sample.node_key),
                    "duration_us": sample.duration_us,
                    "callback_us": sample.callback_us,
                    "presentation_us": sample.presentation_us,
                    "violation": sample.violation,
                    "chaos_enabled": sample.chaos_enabled,
                    "restore_verified": sample.restore_verified,
                    "failure_flags": {
                        "viewport": sample.viewport_changed,
                        "scissor": sample.scissor_changed,
                        "blend": sample.blend_changed,
                        "active_texture": sample.active_texture_changed,
                        "framebuffer_binding": sample.framebuffer_binding_changed,
                    },
                    "bridge_path": sample.bridge_path,
                    "tile_rect_px": {
                        "x": sample.tile_rect_px[0],
                        "y": sample.tile_rect_px[1],
                        "width": sample.tile_rect_px[2],
                        "height": sample.tile_rect_px[3],
                    },
                    "render_size_px": {
                        "width": sample.render_size_px[0],
                        "height": sample.render_size_px[1],
                    },
                    "before": {
                        "viewport": sample.before.viewport,
                        "scissor_enabled": sample.before.scissor_enabled,
                        "blend_enabled": sample.before.blend_enabled,
                        "active_texture": sample.before.active_texture,
                        "framebuffer_binding": sample.before.framebuffer_binding,
                    },
                    "after": {
                        "viewport": sample.after.viewport,
                        "scissor_enabled": sample.after.scissor_enabled,
                        "blend_enabled": sample.after.blend_enabled,
                        "active_texture": sample.after.active_texture,
                        "framebuffer_binding": sample.after.framebuffer_binding,
                    },
                })
            })
            .collect();

        let frames: Vec<Value> = self
            .compositor_state
            .frames
            .iter()
            .map(|frame| {
                let tiles: Vec<Value> = frame
                    .tiles
                    .iter()
                    .map(|tile| {
                        json!({
                            "node_key": format!("{:?}", tile.node_key),
                            "rect": {
                                "min": {"x": tile.rect.min.x, "y": tile.rect.min.y},
                                "max": {"x": tile.rect.max.x, "y": tile.rect.max.y},
                            },
                            "mapped_webview": tile.mapped_webview,
                            "has_context": tile.has_context,
                            "paint_callback_registered": tile.paint_callback_registered,
                            "render_path_hint": tile.render_path_hint,
                        })
                    })
                    .collect();
                let hierarchy: Vec<Value> = frame
                    .hierarchy
                    .iter()
                    .map(|item| {
                        json!({
                            "line": item.line,
                            "node_key": item.node_key.map(|k| format!("{:?}", k)),
                        })
                    })
                    .collect();

                json!({
                    "sequence": frame.sequence,
                    "active_tile_count": frame.active_tile_count,
                    "focused_node_present": frame.focused_node_present,
                    "viewport_rect": {
                        "min": {"x": frame.viewport_rect.min.x, "y": frame.viewport_rect.min.y},
                        "max": {"x": frame.viewport_rect.max.x, "y": frame.viewport_rect.max.y},
                    },
                    "hierarchy": hierarchy,
                    "tiles": tiles,
                })
            })
            .collect();

        json!({
            "version": 1,
            "generated_at_unix_secs": Self::export_timestamp_secs(),
            "event_ring_len": self.event_ring.len(),
            "channels": {
                "message_counts": self.diagnostic_graph.message_counts,
                "message_bytes_sent": self.diagnostic_graph.message_bytes_sent,
                "message_latency_us": self.diagnostic_graph.message_latency_us,
                "message_latency_samples": self.diagnostic_graph.message_latency_samples,
                "message_latency_recent_us": self
                    .diagnostic_graph
                    .message_latency_recent_us
                    .iter()
                    .map(|(channel, values)| (*channel, values.iter().copied().collect::<Vec<_>>()))
                    .collect::<HashMap<_, _>>(),
            },
            "spans": {
                "enter_counts": self.diagnostic_graph.span_enter_counts,
                "exit_counts": self.diagnostic_graph.span_exit_counts,
                "last_duration_us": self.diagnostic_graph.last_span_duration_us,
            },
            "compositor_differential": self.compositor_differential_summary(),
            "compositor_replay": self.compositor_replay_summary(),
            "compositor_frames": frames,
            "compositor_replay_samples": replay_samples,
            "recent_intents": self.intents.iter().map(|intent| {
                json!({
                    "line": intent.line,
                    "cause": intent.cause.map(|c| format!("{:?}", c)),
                })
            }).collect::<Vec<_>>(),
        })
    }

    #[cfg(test)]
    pub(crate) fn force_drain_for_tests(&mut self) {
        self.last_drain_at = Instant::now() - self.drain_interval;
        self.tick_drain();
    }

    #[cfg(test)]
    pub(crate) fn snapshot_json_for_tests(&self) -> Value {
        self.snapshot_json_value()
    }

    #[cfg(test)]
    pub(crate) fn analyzer_snapshots_for_tests(&self) -> Vec<AnalyzerSnapshot> {
        self.analyzer_snapshots()
    }

    #[cfg(test)]
    pub(crate) fn emit_message_sent_for_tests(&self, channel_id: &'static str, byte_len: usize) {
        let _ = self.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id,
            byte_len,
        });
    }

    #[cfg(test)]
    pub(crate) fn emit_message_received_for_tests(
        &self,
        channel_id: &'static str,
        latency_us: u64,
    ) {
        let _ = self.event_tx.send(DiagnosticEvent::MessageReceived {
            channel_id,
            latency_us,
        });
    }

    fn engine_svg(&self) -> String {
        let servo_runtime = (100.0_f32, 40.0_f32);
        let semantic = (100.0_f32, 110.0_f32);
        let intents = (100.0_f32, 260.0_f32);
        let render_pass = (310.0_f32, 170.0_f32);
        let compositor = (520.0_f32, 110.0_f32);
        let backpressure = (520.0_f32, 250.0_f32);

        let mut svg = String::from(
            r#"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"360\" viewBox=\"0 0 640 360\">"#,
        );
        svg.push_str("<rect x=\"0\" y=\"0\" width=\"640\" height=\"360\" fill=\"#0c1018\"/>");

        let mut draw_edge = |from: (f32, f32), to: (f32, f32), metric: EdgeMetric| {
            let width = 1.0 + (metric.count as f32).ln_1p().clamp(0.0, 4.0);
            let color = if metric.bottleneck {
                "#ff6464"
            } else if metric.count > 0 {
                "#50f0c8"
            } else {
                "#3c505a"
            };
            let mid_x = (from.0 + to.0) * 0.5;
            let mid_y = (from.1 + to.1) * 0.5 - 8.0;
            let label = format!(
                "{} | {} {:.1}ms",
                metric.count,
                self.latency_percentile.label(),
                metric.percentile_latency_us as f64 / 1000.0
            );
            svg.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.2}\"/>",
                from.0, from.1, to.0, to.1, color, width
            ));
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{}\" font-size=\"11\" text-anchor=\"middle\">{}</text>",
                mid_x, mid_y, color, label
            ));
        };

        draw_edge(
            servo_runtime,
            semantic,
            self.edge_metric(&CHANNELS_SERVO_TO_SEMANTIC),
        );
        draw_edge(
            semantic,
            intents,
            self.edge_metric(&CHANNELS_SEMANTIC_TO_INTENTS),
        );
        draw_edge(
            intents,
            render_pass,
            self.edge_metric(&CHANNELS_INTENTS_TO_RENDER_PASS),
        );
        draw_edge(
            render_pass,
            compositor,
            self.edge_metric(&CHANNELS_RENDER_PASS_TO_COMPOSITOR),
        );
        draw_edge(
            backpressure,
            intents,
            self.edge_metric(&CHANNELS_BACKPRESSURE_TO_INTENTS),
        );
        draw_edge(
            intents,
            compositor,
            self.edge_metric(&CHANNELS_INTENTS_TO_COMPOSITOR),
        );

        let mut draw_node = |center: (f32, f32), label: &str| {
            let x = center.0 - 55.0;
            let y = center.1 - 18.0;
            svg.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"110\" height=\"36\" rx=\"5\" ry=\"5\" fill=\"#141e2e\" stroke=\"#5adcff\" stroke-width=\"1.2\"/>",
                x, y
            ));
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#d2f0ff\" font-size=\"12\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>",
                center.0, center.1, label
            ));
        };

        draw_node(servo_runtime, "Servo Runtime");
        draw_node(semantic, "Semantic Ingress");
        draw_node(intents, "Intent Pipeline");
        draw_node(render_pass, "Render Pass");
        draw_node(compositor, "Compositor");
        draw_node(backpressure, "Backpressure");

        svg.push_str("</svg>");
        svg
    }

    pub(crate) fn export_snapshot_json(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!(
            "diagnostics-{}.json",
            Self::export_timestamp_secs()
        ));
        let payload = serde_json::to_string_pretty(&self.snapshot_json_value())
            .map_err(|e| format!("failed to serialize diagnostics JSON: {e}"))?;
        fs::write(&path, payload)
            .map_err(|e| format!("failed to write diagnostics JSON {}: {e}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn export_snapshot_svg(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!("diagnostics-{}.svg", Self::export_timestamp_secs()));
        fs::write(&path, self.engine_svg())
            .map_err(|e| format!("failed to write diagnostics SVG {}: {e}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn export_bridge_spike_json(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!("bridge-spike-{}.json", Self::export_timestamp_secs()));
        let payload = serde_json::to_string_pretty(&self.bridge_spike_measurement_value())
            .map_err(|e| format!("failed to serialize bridge spike JSON: {e}"))?;
        fs::write(&path, payload)
            .map_err(|e| format!("failed to write bridge spike JSON {}: {e}", path.display()))?;
        Ok(path)
    }

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

    pub(crate) fn highlighted_tile_node(&self) -> Option<NodeKey> {
        self.pinned_node_key.or(self.hovered_node_key)
    }

    pub(crate) fn take_pending_focus_node(&mut self) -> Option<NodeKey> {
        self.pending_focus_node.take()
    }

    pub(crate) fn render_in_pane(&mut self, ui: &mut egui::Ui, graph_app: &mut GraphBrowserApp) {
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
                let analyzer_snapshots = self.analyzer_snapshots();
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
                ui.small("Save Snapshot JSON includes compositor replay artifacts and path details.");
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
                        ui.monospace(
                            differential["fallback_signature_changed_count"].to_string(),
                        );
                        ui.end_row();

                        ui.monospace("computed_skip_rate_basis_points");
                        ui.monospace(
                            differential["computed_skip_rate_basis_points"].to_string(),
                        );
                        ui.end_row();

                        ui.monospace("content_culled_offviewport_count");
                        ui.monospace(
                            differential["content_culled_offviewport_count"].to_string(),
                        );
                        ui.end_row();

                        ui.monospace("degradation_gpu_pressure_count");
                        ui.monospace(
                            differential["degradation_gpu_pressure_count"].to_string(),
                        );
                        ui.end_row();

                        ui.monospace("degradation_placeholder_mode_count");
                        ui.monospace(
                            differential["degradation_placeholder_mode_count"].to_string(),
                        );
                        ui.end_row();

                        ui.monospace("resource_reuse_context_hit_count");
                        ui.monospace(
                            differential["resource_reuse_context_hit_count"].to_string(),
                        );
                        ui.end_row();

                        ui.monospace("resource_reuse_context_miss_count");
                        ui.monospace(
                            differential["resource_reuse_context_miss_count"].to_string(),
                        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case(LatencyPercentile::P90, 90_000)]
    #[case(LatencyPercentile::P95, 95_000)]
    #[case(LatencyPercentile::P99, 99_000)]
    fn edge_metric_respects_selected_percentile(
        #[case] percentile: LatencyPercentile,
        #[case] expected_us: u64,
    ) {
        let mut state = DiagnosticsState::new();
        state.latency_percentile = percentile;
        state.bottleneck_latency_us = 80_000;

        let channel = CHANNELS_INTENTS_TO_RENDER_PASS[0];
        let samples = state
            .diagnostic_graph
            .message_latency_recent_us
            .entry(channel)
            .or_default();
        for value in 1_u64..=100 {
            samples.push_back(value * 1000);
        }
        state.diagnostic_graph.message_counts.insert(channel, 100);

        let metric = state.edge_metric(&CHANNELS_INTENTS_TO_RENDER_PASS);
        assert_eq!(metric.percentile_latency_us, expected_us);
        assert!(metric.bottleneck);
    }

    #[rstest]
    #[case(40_000, true)]
    #[case(100_000, false)]
    fn edge_metric_bottleneck_threshold_is_configurable(
        #[case] threshold_us: u64,
        #[case] expected_bottleneck: bool,
    ) {
        let mut state = DiagnosticsState::new();
        state.latency_percentile = LatencyPercentile::P95;
        state.bottleneck_latency_us = threshold_us;

        let channel = CHANNELS_RENDER_PASS_TO_COMPOSITOR[0];
        let samples = state
            .diagnostic_graph
            .message_latency_recent_us
            .entry(channel)
            .or_default();
        for _ in 0..64 {
            samples.push_back(50_000);
        }
        state.diagnostic_graph.message_counts.insert(channel, 64);

        let metric = state.edge_metric(&CHANNELS_RENDER_PASS_TO_COMPOSITOR);
        assert_eq!(metric.percentile_latency_us, 50_000);
        assert_eq!(metric.bottleneck, expected_bottleneck);
    }

    #[test]
    fn percentile_95_uses_upper_percentile_rank() {
        let mut values = vec![1_u64, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let p95 = DiagnosticsState::percentile_95(&mut values);
        assert_eq!(p95, 10);
    }

    #[test]
    fn tick_drain_respects_10hz_interval_gate() {
        let mut state = DiagnosticsState::new();
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: "test.channel",
            byte_len: 42,
        });

        state.last_drain_at = Instant::now();
        state.tick_drain();
        assert_eq!(
            state
                .diagnostic_graph
                .message_counts
                .get("test.channel")
                .copied()
                .unwrap_or(0),
            0
        );

        state.last_drain_at = Instant::now() - state.drain_interval;
        state.tick_drain();
        assert_eq!(
            state
                .diagnostic_graph
                .message_counts
                .get("test.channel")
                .copied()
                .unwrap_or(0),
            1
        );
    }

    fn test_active_analyzer(
        _graph: &DiagnosticGraph,
        _event_ring: &VecDeque<DiagnosticEvent>,
    ) -> AnalyzerResult {
        AnalyzerResult {
            signal: AnalyzerSignal::Active,
            summary: "test analyzer active".to_string(),
        }
    }

    #[test]
    fn analyzer_registry_registration_surface_rejects_duplicate_ids() {
        let mut state = DiagnosticsState::new();

        assert!(state.register_analyzer(
            "diagnostics.test.analyzer",
            "Test Analyzer",
            test_active_analyzer,
        ));
        assert!(!state.register_analyzer(
            "diagnostics.test.analyzer",
            "Duplicate Analyzer",
            test_active_analyzer,
        ));
    }

    #[test]
    fn analyzer_registry_executes_registered_analyzers_on_drain_cycle() {
        let mut state = DiagnosticsState::new();
        let _ = state.register_analyzer(
            "diagnostics.test.analyzer",
            "Test Analyzer",
            test_active_analyzer,
        );

        state.force_drain_for_tests();
        let snapshots = state.analyzer_snapshots_for_tests();

        let event_ring_pressure = snapshots
            .iter()
            .find(|snapshot| snapshot.id == "diagnostics.event_ring_pressure")
            .expect("builtin analyzer should be registered");
        assert!(event_ring_pressure.run_count >= 1);
        assert!(event_ring_pressure.last_result.is_some());

        let custom = snapshots
            .iter()
            .find(|snapshot| snapshot.id == "diagnostics.test.analyzer")
            .expect("custom analyzer should be registered");
        assert!(custom.run_count >= 1);
        let result = custom
            .last_result
            .as_ref()
            .expect("custom analyzer should produce result");
        assert_eq!(result.signal, AnalyzerSignal::Active);
        assert_eq!(result.summary, "test analyzer active");
    }

    #[test]
    fn config_changed_event_uses_registry_channel_and_payload() {
        let config = diagnostics_registry::ChannelConfig {
            enabled: false,
            sample_rate: 0.25,
            retention_count: 321,
        };
        let channel_id = "mod.graphshell.render.debug";

        let event = config_changed_event(channel_id, &config);
        let expected_len = config_changed_payload_len(channel_id, &config);

        match event {
            DiagnosticEvent::MessageSent {
                channel_id,
                byte_len,
            } => {
                assert_eq!(channel_id, CHANNEL_DIAGNOSTICS_CONFIG_CHANGED);
                assert_eq!(byte_len, expected_len);
                assert!(byte_len > 0);
            }
            other => panic!("unexpected event variant: {other:?}"),
        }
    }

    #[test]
    fn snapshot_json_contains_core_sections() {
        let mut state = DiagnosticsState::new();
        let node_key = NodeKey::new(1);
        state.push_frame(CompositorFrameSample {
            sequence: 7,
            active_tile_count: 1,
            focused_node_present: true,
            viewport_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 80.0)),
            hierarchy: vec![HierarchySample {
                line: "* TileId(1) Node Viewer NodeKey(1)".to_string(),
                node_key: Some(node_key),
            }],
            tiles: vec![CompositorTileSample {
                node_key,
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(50.0, 40.0)),
                mapped_webview: true,
                has_context: true,
                paint_callback_registered: true,
                render_path_hint: "composited",
            }],
        });
        state.record_intents(&[GraphIntent::ToggleHelpPanel]);
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: "snapshot.test.channel",
            byte_len: 9,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageReceived {
            channel_id: "snapshot.test.channel",
            latency_us: 2_500,
        });
        state.last_drain_at = Instant::now() - state.drain_interval;
        state.tick_drain();

        let snapshot = state.snapshot_json_value();
        assert_eq!(snapshot["version"].as_u64(), Some(1));
        assert!(snapshot["channels"].is_object());
        assert!(snapshot["spans"].is_object());
        assert!(snapshot["compositor_frames"].is_array());
        assert!(snapshot["recent_intents"].is_array());
        assert_eq!(
            snapshot["compositor_frames"][0]["sequence"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["recent_intents"].as_array().map(|v| v.len()),
            Some(1)
        );
    }

    #[test]
    fn snapshot_json_channel_counts_match_aggregates() {
        let mut state = DiagnosticsState::new();
        let channel = "parity.channel";

        for _ in 0..3 {
            let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
                channel_id: channel,
                byte_len: 11,
            });
        }
        let _ = state.event_tx.send(DiagnosticEvent::MessageReceived {
            channel_id: channel,
            latency_us: 7_000,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageReceived {
            channel_id: channel,
            latency_us: 13_000,
        });
        state.last_drain_at = Instant::now() - state.drain_interval;
        state.tick_drain();

        let snapshot = state.snapshot_json_value();
        let json_count = snapshot["channels"]["message_counts"][channel]
            .as_u64()
            .unwrap_or(0);
        let json_bytes = snapshot["channels"]["message_bytes_sent"][channel]
            .as_u64()
            .unwrap_or(0);
        let json_latency_sum = snapshot["channels"]["message_latency_us"][channel]
            .as_u64()
            .unwrap_or(0);
        let json_latency_samples = snapshot["channels"]["message_latency_samples"][channel]
            .as_u64()
            .unwrap_or(0);

        assert_eq!(
            json_count,
            state
                .diagnostic_graph
                .message_counts
                .get(channel)
                .copied()
                .unwrap_or(0)
        );
        assert_eq!(
            json_bytes,
            state
                .diagnostic_graph
                .message_bytes_sent
                .get(channel)
                .copied()
                .unwrap_or(0)
        );
        assert_eq!(
            json_latency_sum,
            state
                .diagnostic_graph
                .message_latency_us
                .get(channel)
                .copied()
                .unwrap_or(0)
        );
        assert_eq!(
            json_latency_samples,
            state
                .diagnostic_graph
                .message_latency_samples
                .get(channel)
                .copied()
                .unwrap_or(0)
        );
    }

    #[test]
    fn diagnostics_json_snapshot_shape_is_stable() {
        let mut state = DiagnosticsState::new();
        let node_key = NodeKey::new(7);
        state.push_frame(CompositorFrameSample {
            sequence: 88,
            active_tile_count: 1,
            focused_node_present: true,
            viewport_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(120.0, 100.0)),
            hierarchy: vec![HierarchySample {
                line: "* TileId(2) Node Viewer NodeKey(7)".to_string(),
                node_key: Some(node_key),
            }],
            tiles: vec![CompositorTileSample {
                node_key,
                rect: egui::Rect::from_min_max(egui::pos2(4.0, 6.0), egui::pos2(80.0, 70.0)),
                mapped_webview: true,
                has_context: true,
                paint_callback_registered: true,
                render_path_hint: "composited",
            }],
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: "snapshot.shape",
            byte_len: 3,
        });
        state.record_intents(&[GraphIntent::ToggleHelpPanel]);
        state.last_drain_at = Instant::now() - state.drain_interval;
        state.tick_drain();

        let mut json = state.snapshot_json_value();
        json["generated_at_unix_secs"] = serde_json::json!("[unix-secs]");
        if let Some(frames) = json["compositor_frames"].as_array_mut() {
            for frame in frames {
                frame["sequence"] = serde_json::json!("[sequence]");
            }
        }

        let top_level_keys = json
            .as_object()
            .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let channel_keys = json["channels"]
            .as_object()
            .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let shape = serde_json::json!({
            "top_level_keys": top_level_keys,
            "channel_keys": channel_keys,
            "frame_count": json["compositor_frames"].as_array().map(|v| v.len()).unwrap_or(0),
            "intent_count": json["recent_intents"].as_array().map(|v| v.len()).unwrap_or(0),
            "generated_at_unix_secs": json["generated_at_unix_secs"],
            "first_frame_sequence": json["compositor_frames"][0]["sequence"],
        });
        insta::assert_debug_snapshot!(shape, @r###"
Object {
    "top_level_keys": Array [
        String("version"),
        String("generated_at_unix_secs"),
        String("event_ring_len"),
        String("channels"),
        String("spans"),
        String("compositor_differential"),
        String("compositor_replay"),
        String("compositor_frames"),
        String("compositor_replay_samples"),
        String("recent_intents"),
    ],
    "channel_keys": Array [
        String("message_counts"),
        String("message_bytes_sent"),
        String("message_latency_us"),
        String("message_latency_samples"),
        String("message_latency_recent_us"),
    ],
    "frame_count": Number(1),
    "intent_count": Number(1),
    "generated_at_unix_secs": String("[unix-secs]"),
    "first_frame_sequence": String("[sequence]"),
}
        "###);
    }

    #[test]
    fn replay_export_feedback_includes_path_and_counts() {
        let path = PathBuf::from("diagnostics-123.json");
        let samples = vec![
            CompositorReplaySample {
                sequence: 1,
                node_key: NodeKey::new(1),
                duration_us: 7,
                callback_us: 7,
                presentation_us: 7,
                violation: false,
                bridge_path: "test.bridge",
                bridge_mode: "test.bridge_mode",
                tile_rect_px: [0, 0, 1, 1],
                render_size_px: [1, 1],
                chaos_enabled: false,
                restore_verified: true,
                viewport_changed: false,
                scissor_changed: false,
                blend_changed: false,
                active_texture_changed: false,
                framebuffer_binding_changed: false,
                before: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 1, 1],
                    scissor_enabled: false,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
                after: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 1, 1],
                    scissor_enabled: false,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
            },
            CompositorReplaySample {
                sequence: 2,
                node_key: NodeKey::new(2),
                duration_us: 11,
                callback_us: 11,
                presentation_us: 11,
                violation: true,
                bridge_path: "test.bridge",
                bridge_mode: "test.bridge_mode",
                tile_rect_px: [1, 2, 3, 4],
                render_size_px: [3, 4],
                chaos_enabled: false,
                restore_verified: true,
                viewport_changed: true,
                scissor_changed: true,
                blend_changed: false,
                active_texture_changed: true,
                framebuffer_binding_changed: true,
                before: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 1, 1],
                    scissor_enabled: false,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
                after: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 2, 2],
                    scissor_enabled: true,
                    blend_enabled: false,
                    active_texture: 1,
                    framebuffer_binding: 1,
                },
            },
        ];

        let feedback = DiagnosticsState::replay_export_feedback(&path, &samples);
        assert!(feedback.contains("diagnostics-123.json"));
        assert!(feedback.contains("replay samples: 2"));
        assert!(feedback.contains("violations: 1"));
    }

    #[test]
    fn bridge_spike_measurement_payload_contains_contract_fields() {
        let samples = vec![
            CompositorReplaySample {
                sequence: 1,
                node_key: NodeKey::new(1),
                duration_us: 30,
                callback_us: 20,
                presentation_us: 10,
                violation: false,
                bridge_path: "gl.render_to_parent_callback",
                bridge_mode: "glow_callback",
                tile_rect_px: [0, 0, 64, 64],
                render_size_px: [64, 64],
                chaos_enabled: false,
                restore_verified: true,
                viewport_changed: false,
                scissor_changed: false,
                blend_changed: false,
                active_texture_changed: false,
                framebuffer_binding_changed: false,
                before: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 64, 64],
                    scissor_enabled: false,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
                after: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 64, 64],
                    scissor_enabled: false,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
            },
            CompositorReplaySample {
                sequence: 2,
                node_key: NodeKey::new(2),
                duration_us: 45,
                callback_us: 25,
                presentation_us: 20,
                violation: true,
                bridge_path: "gl.render_to_parent_callback",
                bridge_mode: "glow_callback",
                tile_rect_px: [4, 8, 120, 80],
                render_size_px: [120, 80],
                chaos_enabled: false,
                restore_verified: false,
                viewport_changed: true,
                scissor_changed: true,
                blend_changed: false,
                active_texture_changed: false,
                framebuffer_binding_changed: false,
                before: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 120, 80],
                    scissor_enabled: false,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
                after: crate::shell::desktop::workbench::compositor_adapter::GlStateSnapshot {
                    viewport: [0, 0, 120, 80],
                    scissor_enabled: true,
                    blend_enabled: false,
                    active_texture: 0,
                    framebuffer_binding: 0,
                },
            },
        ];

        let payload = DiagnosticsState::bridge_spike_measurement_value_from_samples(&samples);
        assert_eq!(payload["measurement_contract"]["sample_count"].as_u64(), Some(2));
        assert_eq!(
            payload["measurement_contract"]["failed_frame_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            payload["measurement_contract"]["avg_callback_us"].as_u64(),
            Some(22)
        );
        assert_eq!(
            payload["measurement_contract"]["avg_presentation_us"].as_u64(),
            Some(15)
        );
        assert_eq!(
            payload["measurement_contract"]["chaos_enabled_sample_count"].as_u64(),
            Some(0)
        );
        assert_eq!(
            payload["measurement_contract"]["restore_verification_fail_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            payload["measurement_contract"]["failed_by_reason"]["viewport"].as_u64(),
            Some(1)
        );
        assert_eq!(
            payload["measurement_contract"]["failed_by_reason"]["scissor"].as_u64(),
            Some(1)
        );
        assert_eq!(
            payload["measurement_contract"]["latest"]["bridge_path"].as_str(),
            Some("gl.render_to_parent_callback")
        );
        assert_eq!(
            payload["measurement_contract"]["latest"]["bridge_mode"].as_str(),
            Some("glow_callback")
        );
        assert!(payload["samples"].is_array());
    }

    #[test]
    fn snapshot_json_includes_compositor_differential_summary_section() {
        let mut state = DiagnosticsState::new();
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE,
            byte_len: 5000,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE,
            byte_len: 4,
        });
        state.force_drain_for_tests();

        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["compositor_differential"]["content_composed_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["content_skipped_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["computed_skip_rate_basis_points"].as_u64(),
            Some(5000)
        );
        assert_eq!(
            snapshot["compositor_differential"]["avg_skip_rate_basis_points"].as_u64(),
            Some(5000)
        );
        assert_eq!(
            snapshot["compositor_differential"]["content_culled_offviewport_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["degradation_gpu_pressure_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["degradation_placeholder_mode_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["resource_reuse_context_hit_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["resource_reuse_context_miss_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["overlay_batch_sample_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_differential"]["avg_overlay_batch_size"].as_u64(),
            Some(4)
        );
    }

    #[test]
    fn snapshot_json_includes_compositor_render_path_hint() {
        let mut state = DiagnosticsState::new();
        let node_key = NodeKey::new(9);
        state.push_frame(CompositorFrameSample {
            sequence: 1,
            active_tile_count: 1,
            focused_node_present: false,
            viewport_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(10.0, 10.0)),
            hierarchy: vec![],
            tiles: vec![CompositorTileSample {
                node_key,
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(5.0, 5.0)),
                mapped_webview: true,
                has_context: true,
                paint_callback_registered: true,
                render_path_hint: "composited",
            }],
        });
        state.force_drain_for_tests();
        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["compositor_frames"][0]["tiles"][0]["render_path_hint"].as_str(),
            Some("composited")
        );
    }

    #[test]
    fn snapshot_json_includes_compositor_replay_samples_section() {
        let state = DiagnosticsState::new();
        let snapshot = state.snapshot_json_value();
        assert!(snapshot["compositor_replay_samples"].is_array());
    }

    #[test]
    fn snapshot_json_includes_compositor_bridge_probe_summary_metrics() {
        let mut state = DiagnosticsState::new();
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
            byte_len: 8,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
            byte_len: 50,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
            byte_len: 70,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
            byte_len: 80,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
            byte_len: 100,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
            byte_len: 8,
        });

        state.force_drain_for_tests();
        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["compositor_replay"]["bridge_probe_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["bridge_failed_frame_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["avg_bridge_callback_us"].as_u64(),
            Some(60)
        );
        assert_eq!(
            snapshot["compositor_replay"]["avg_bridge_presentation_us"].as_u64(),
            Some(90)
        );
    }

    #[test]
    fn diagnostics_svg_snapshot_shape_is_stable() {
        let mut state = DiagnosticsState::new();
        let channel = CHANNELS_SEMANTIC_TO_INTENTS[0];
        state.diagnostic_graph.message_counts.insert(channel, 12);
        state.diagnostic_graph.message_latency_recent_us.insert(
            channel,
            VecDeque::from(vec![1000_u64, 2000, 3000, 4000, 5000]),
        );
        state.latency_percentile = LatencyPercentile::P95;
        state.bottleneck_latency_us = 4_000;

        let svg = state.engine_svg();
        let shape = serde_json::json!({
                "starts_with_svg": svg.starts_with("<svg"),
            "contains_servo_runtime": svg.contains("Servo Runtime"),
                "contains_semantic": svg.contains("Semantic Ingress"),
                "contains_intent_pipeline": svg.contains("Intent Pipeline"),
                "contains_render_pass": svg.contains("Render Pass"),
                "contains_percentile_label": svg.contains(state.latency_percentile.label()),
                "line_count": svg.matches("<line ").count(),
                "text_count": svg.matches("<text ").count(),
        });
        insta::assert_debug_snapshot!(shape, @r###"
                Object {
                    "starts_with_svg": Bool(true),
                    "contains_servo_runtime": Bool(true),
                    "contains_semantic": Bool(true),
                    "contains_intent_pipeline": Bool(true),
                    "contains_render_pass": Bool(true),
                    "contains_percentile_label": Bool(true),
                    "line_count": Number(6),
                    "text_count": Number(12),
                }
                "###);
    }

    #[derive(Clone, Debug)]
    enum AggregateEvent {
        Sent { bytes: usize },
        Received { latency_us: u64 },
    }

    fn aggregate_event_strategy() -> impl Strategy<Value = AggregateEvent> {
        prop_oneof![
            (1_usize..200).prop_map(|bytes| AggregateEvent::Sent { bytes }),
            (1_u64..200_000).prop_map(|latency_us| AggregateEvent::Received { latency_us }),
        ]
    }

    proptest! {
        #[test]
        fn proptest_tick_drain_aggregation_matches_event_stream(
            events in prop::collection::vec(aggregate_event_strategy(), 0..120)
        ) {
            let mut state = DiagnosticsState::new();
            let channel = "proptest.aggregate.channel";

            let mut expected_count = 0_u64;
            let mut expected_bytes = 0_u64;
            let mut expected_latency_sum = 0_u64;
            let mut expected_latency_samples = 0_u64;

            for event in &events {
                match event {
                    AggregateEvent::Sent { bytes } => {
                        expected_count += 1;
                        expected_bytes += *bytes as u64;
                        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
                            channel_id: channel,
                            byte_len: *bytes,
                        });
                    }
                    AggregateEvent::Received { latency_us } => {
                        expected_count += 1;
                        expected_latency_sum += *latency_us;
                        expected_latency_samples += 1;
                        let _ = state.event_tx.send(DiagnosticEvent::MessageReceived {
                            channel_id: channel,
                            latency_us: *latency_us,
                        });
                    }
                }
            }

            state.last_drain_at = Instant::now() - state.drain_interval;
            state.tick_drain();

            prop_assert_eq!(
                state.diagnostic_graph.message_counts.get(channel).copied().unwrap_or(0),
                expected_count
            );
            prop_assert_eq!(
                state.diagnostic_graph.message_bytes_sent.get(channel).copied().unwrap_or(0),
                expected_bytes
            );
            prop_assert_eq!(
                state.diagnostic_graph.message_latency_us.get(channel).copied().unwrap_or(0),
                expected_latency_sum
            );
            prop_assert_eq!(
                state.diagnostic_graph.message_latency_samples.get(channel).copied().unwrap_or(0),
                expected_latency_samples
            );
        }
    }
}
