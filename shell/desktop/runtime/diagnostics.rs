/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet, VecDeque};
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
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT, CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
    CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE, CHANNEL_COMPOSITOR_GL_STATE_VIOLATION,
    CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI, CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER, CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE, CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION,
    CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED, CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
    CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT, CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS,
    CHANNEL_DIAGNOSTICS_CONFIG_CHANGED, CHANNEL_IDENTITY_SIGN_FAILED,
    CHANNEL_IDENTITY_TRUST_STORE_LOAD_FAILED, CHANNEL_IDENTITY_VERIFY_FAILED,
    CHANNEL_INVARIANT_TIMEOUT, CHANNEL_PERSISTENCE_RECOVER_FAILED,
    CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED, CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED,
    CHANNEL_REGISTER_SIGNAL_ROUTING_LAGGED, CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED,
    CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH, CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED, CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT, CHANNEL_STARTUP_SELFCHECK_CHANNELS_COMPLETE,
    CHANNEL_STARTUP_SELFCHECK_CHANNELS_INCOMPLETE, CHANNEL_STARTUP_SELFCHECK_REGISTRIES_LOADED,
    CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION, CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK,
    CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH, CHANNEL_UX_NAVIGATION_TRANSITION,
    CHANNEL_UX_NAVIGATION_VIOLATION, CHANNEL_VERSE_SYNC_ACCESS_DENIED,
    CHANNEL_VIEWER_FALLBACK_USED, CHANNEL_VIEWER_SELECT_STARTED, CHANNEL_VIEWER_SELECT_SUCCEEDED,
};
use crate::shell::desktop::runtime::tracing::perf_ring_snapshot;
use crate::shell::desktop::ui::gui_state::RuntimeFocusInspector;
use crate::shell::desktop::workbench::compositor_adapter::{
    CompositorReplaySample, replay_samples_snapshot,
};
use crate::shell::desktop::workbench::pane_model::TileRenderMode;

#[path = "diagnostics/export.rs"]
mod export;
#[path = "diagnostics/pane_ui.rs"]
mod pane_ui;

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
    Analysis,
    Compositor,
    Intents,
}

#[derive(Clone, Debug)]
pub(crate) struct CompositorTileSample {
    pub(crate) pane_id: String,
    pub(crate) node_key: NodeKey,
    pub(crate) render_mode: TileRenderMode,
    pub(crate) estimated_content_bytes: usize,
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

type DiagnosticsAnalyzerFn =
    fn(&DiagnosticGraph, &VecDeque<DiagnosticEvent>, &Value) -> AnalyzerResult;

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

#[derive(Clone, Debug)]
pub(crate) struct LaneChannelSummary {
    pub(crate) lane_id: &'static str,
    pub(crate) analyzer_id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) signal: AnalyzerSignal,
    pub(crate) summary: String,
    pub(crate) channel_counts: Vec<(&'static str, u64)>,
}

#[derive(Clone, Debug)]
pub(crate) struct ChannelTrendSummary {
    pub(crate) channel_id: &'static str,
    pub(crate) message_count: u64,
    pub(crate) avg_latency_us: u64,
    pub(crate) trend: &'static str,
    pub(crate) recent_samples_us: Vec<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ChannelEventReceipt {
    pub(crate) channel_id: &'static str,
    pub(crate) direction: &'static str,
    pub(crate) detail: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ChannelHistorySummary {
    pub(crate) channel_id: &'static str,
    pub(crate) count_buckets: Vec<u64>,
    pub(crate) latency_buckets_us: Vec<u64>,
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

    fn run_all(
        &mut self,
        graph: &DiagnosticGraph,
        event_ring: &VecDeque<DiagnosticEvent>,
        tracing_perf_snapshot: &Value,
    ) {
        for analyzer in &mut self.analyzers {
            analyzer.run_count = analyzer.run_count.saturating_add(1);
            analyzer.last_result =
                Some((analyzer.analyze)(graph, event_ring, tracing_perf_snapshot));
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
    _tracing_perf_snapshot: &Value,
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

fn analyze_startup_structural_selfcheck(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let incomplete_count = graph
        .message_counts
        .get(CHANNEL_STARTUP_SELFCHECK_CHANNELS_INCOMPLETE)
        .copied()
        .unwrap_or(0);
    if incomplete_count > 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Alert,
            summary: format!(
                "startup self-check found incomplete channel contract ({incomplete_count} findings)"
            ),
        };
    }

    let registries_loaded = graph
        .message_counts
        .get(CHANNEL_STARTUP_SELFCHECK_REGISTRIES_LOADED)
        .copied()
        .unwrap_or(0);
    let channels_complete = graph
        .message_counts
        .get(CHANNEL_STARTUP_SELFCHECK_CHANNELS_COMPLETE)
        .copied()
        .unwrap_or(0);

    if registries_loaded > 0 && channels_complete > 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Active,
            summary: "startup self-check passed (registries loaded, channels complete)".to_string(),
        };
    }

    AnalyzerResult {
        signal: AnalyzerSignal::Quiet,
        summary: "startup self-check pending first drain".to_string(),
    }
}

fn analyze_tracing_hotpath_latency(
    _graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let sample_count = tracing_perf_snapshot["sample_count"].as_u64().unwrap_or(0);
    if sample_count == 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Quiet,
            summary: "tracing perf ring idle (no samples yet)".to_string(),
        };
    }

    let p95_elapsed_us = tracing_perf_snapshot["p95_elapsed_us"]
        .as_u64()
        .unwrap_or(0);
    let avg_elapsed_us = tracing_perf_snapshot["avg_elapsed_us"]
        .as_u64()
        .unwrap_or(0);
    let max_elapsed_us = tracing_perf_snapshot["max_elapsed_us"]
        .as_u64()
        .unwrap_or(0);

    let (signal, status) = if p95_elapsed_us >= 16_000 {
        (AnalyzerSignal::Alert, "hotpath p95 above 16ms")
    } else if p95_elapsed_us >= 8_000 {
        (AnalyzerSignal::Active, "hotpath p95 elevated")
    } else {
        (AnalyzerSignal::Active, "hotpath latency nominal")
    };

    AnalyzerResult {
        signal,
        summary: format!(
            "{status} (samples={sample_count}, avg={}us, p95={}us, max={}us)",
            avg_elapsed_us, p95_elapsed_us, max_elapsed_us
        ),
    }
}

fn analyze_render_mode_health(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let composited = graph
        .message_counts
        .get(CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE)
        .copied()
        .unwrap_or(0);
    let native = graph
        .message_counts
        .get(CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY)
        .copied()
        .unwrap_or(0);
    let embedded = graph
        .message_counts
        .get(CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI)
        .copied()
        .unwrap_or(0);
    let placeholder_mode = graph
        .message_counts
        .get(CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER)
        .copied()
        .unwrap_or(0);
    let fallback_used = graph
        .message_counts
        .get(CHANNEL_VIEWER_FALLBACK_USED)
        .copied()
        .unwrap_or(0);
    let degraded_placeholder = graph
        .message_counts
        .get(CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE)
        .copied()
        .unwrap_or(0);

    let observed = composited + native + embedded + placeholder_mode;
    if observed == 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Quiet,
            summary: "render-mode health pending first compositor overlay mode samples".to_string(),
        };
    }

    let (signal, status) = if degraded_placeholder > 0 {
        (AnalyzerSignal::Alert, "placeholder degradation observed")
    } else if fallback_used > 0 || placeholder_mode > 0 {
        (
            AnalyzerSignal::Active,
            "fallback or placeholder mode observed",
        )
    } else {
        (AnalyzerSignal::Active, "render-mode mix healthy")
    };

    AnalyzerResult {
        signal,
        summary: format!(
            "{status} (comp={composited}, native={native}, embedded={embedded}, placeholder={placeholder_mode}, fallback={fallback_used})"
        ),
    }
}

fn analyze_signal_routing_health(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let published = graph
        .message_counts
        .get(CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED)
        .copied()
        .unwrap_or(0);
    let unrouted = graph
        .message_counts
        .get(CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED)
        .copied()
        .unwrap_or(0);
    let failed = graph
        .message_counts
        .get(CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED)
        .copied()
        .unwrap_or(0);
    let lagged = graph
        .message_counts
        .get(CHANNEL_REGISTER_SIGNAL_ROUTING_LAGGED)
        .copied()
        .unwrap_or(0);
    let queue_samples = graph
        .message_counts
        .get(CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH)
        .copied()
        .unwrap_or(0);
    let queue_total = graph
        .message_bytes_sent
        .get(CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH)
        .copied()
        .unwrap_or(0);
    let avg_queue_depth = if queue_samples == 0 {
        0
    } else {
        queue_total / queue_samples
    };

    if published == 0 && unrouted == 0 && failed == 0 && lagged == 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Quiet,
            summary: "signal-routing health pending first published signals".to_string(),
        };
    }

    let (signal, status) = if failed > 0 || unrouted > 0 {
        (AnalyzerSignal::Alert, "signal-routing failures detected")
    } else if lagged > 0 || avg_queue_depth > 0 {
        (
            AnalyzerSignal::Active,
            "signal-routing active with backpressure",
        )
    } else {
        (AnalyzerSignal::Active, "signal-routing healthy")
    };

    AnalyzerResult {
        signal,
        summary: format!(
            "{status} (published={published}, unrouted={unrouted}, failed={failed}, lagged={lagged}, avg_queue_depth={avg_queue_depth})"
        ),
    }
}

fn analyze_navigator_projection_health(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let nav_transition = graph
        .message_counts
        .get(CHANNEL_UX_NAVIGATION_TRANSITION)
        .copied()
        .unwrap_or(0);
    let nav_violation = graph
        .message_counts
        .get(CHANNEL_UX_NAVIGATION_VIOLATION)
        .copied()
        .unwrap_or(0);
    let arrangement_health = graph
        .message_counts
        .get(CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH)
        .copied()
        .unwrap_or(0);
    let missing_family = graph
        .message_counts
        .get(CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK)
        .copied()
        .unwrap_or(0);
    let durability = graph
        .message_counts
        .get(CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION)
        .copied()
        .unwrap_or(0);

    let observed =
        nav_transition + nav_violation + arrangement_health + missing_family + durability;
    if observed == 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Quiet,
            summary: "navigator projection health pending first UX navigation signals".to_string(),
        };
    }

    let (signal, status) = if nav_violation > 0 || missing_family > 0 {
        (
            AnalyzerSignal::Alert,
            "navigator/arrangement contract violations detected",
        )
    } else {
        (
            AnalyzerSignal::Active,
            "navigator projection and arrangement signals healthy",
        )
    };

    AnalyzerResult {
        signal,
        summary: format!(
            "{status} (nav_transition={nav_transition}, nav_violation={nav_violation}, projection_health={arrangement_health}, missing_family={missing_family}, durability={durability})"
        ),
    }
}

fn analyze_persistence_health(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let open_failed = graph
        .message_counts
        .get(CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED)
        .copied()
        .unwrap_or(0);
    let open_timeout = graph
        .message_counts
        .get(CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT)
        .copied()
        .unwrap_or(0);
    let recover_failed = graph
        .message_counts
        .get(CHANNEL_PERSISTENCE_RECOVER_FAILED)
        .copied()
        .unwrap_or(0);
    let recover_succeeded = graph
        .message_counts
        .get(CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED)
        .copied()
        .unwrap_or(0);

    if open_failed > 0 || open_timeout > 0 || recover_failed > 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Alert,
            summary: format!(
                "persistence degraded (open_failed={open_failed}, open_timeout={open_timeout}, recover_failed={recover_failed}, recover_succeeded={recover_succeeded})"
            ),
        };
    }

    if recover_succeeded > 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Active,
            summary: format!(
                "persistence healthy (recover_succeeded={recover_succeeded})"
            ),
        };
    }

    AnalyzerResult {
        signal: AnalyzerSignal::Quiet,
        summary: "persistence health pending startup receipts".to_string(),
    }
}

fn analyze_security_identity_health(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let access_denied = graph
        .message_counts
        .get(CHANNEL_VERSE_SYNC_ACCESS_DENIED)
        .copied()
        .unwrap_or(0);
    let sign_failed = graph
        .message_counts
        .get(CHANNEL_IDENTITY_SIGN_FAILED)
        .copied()
        .unwrap_or(0);
    let verify_failed = graph
        .message_counts
        .get(CHANNEL_IDENTITY_VERIFY_FAILED)
        .copied()
        .unwrap_or(0);
    let trust_store_failed = graph
        .message_counts
        .get(CHANNEL_IDENTITY_TRUST_STORE_LOAD_FAILED)
        .copied()
        .unwrap_or(0);

    if access_denied > 0 || sign_failed > 0 || verify_failed > 0 || trust_store_failed > 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Alert,
            summary: format!(
                "security/identity issues observed (access_denied={access_denied}, sign_failed={sign_failed}, verify_failed={verify_failed}, trust_store_failed={trust_store_failed})"
            ),
        };
    }

    AnalyzerResult {
        signal: AnalyzerSignal::Quiet,
        summary: "security/identity health quiet".to_string(),
    }
}

fn analyze_diagnostics_registry_health(
    graph: &DiagnosticGraph,
    _event_ring: &VecDeque<DiagnosticEvent>,
    _tracing_perf_snapshot: &Value,
) -> AnalyzerResult {
    let orphan_channels = diagnostics_registry::list_orphan_channels_snapshot();
    let orphan_count = orphan_channels.len();
    let incomplete = graph
        .message_counts
        .get(CHANNEL_STARTUP_SELFCHECK_CHANNELS_INCOMPLETE)
        .copied()
        .unwrap_or(0);

    if incomplete > 0 || orphan_count > 0 {
        return AnalyzerResult {
            signal: AnalyzerSignal::Alert,
            summary: format!(
                "diagnostics registry drift detected (incomplete={incomplete}, orphan_channels={orphan_count})"
            ),
        };
    }

    AnalyzerResult {
        signal: AnalyzerSignal::Active,
        summary: "diagnostics registry health nominal".to_string(),
    }
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
    (
        "NativeOverlay",
        CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    ),
    (
        "EmbeddedEgui",
        CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI,
    ),
    ("Placeholder", CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER),
];

#[derive(Clone, Debug)]
struct IntentSample {
    line: String,
    cause: Option<LifecycleCause>,
}

fn invoke_minimal_test_harness_entry_point() -> Result<String, String> {
    let required_channels = diagnostics_registry::phase0_required_channels().len()
        + diagnostics_registry::phase2_required_channels().len()
        + diagnostics_registry::phase3_required_channels().len();
    let available_channels = diagnostics_registry::list_channel_configs_snapshot().len();

    if available_channels < required_channels {
        return Err(format!(
            "required channel contracts missing (required={required_channels}, available={available_channels})"
        ));
    }

    Ok(format!(
        "minimal entry point passed (required_channels={required_channels}, available_channels={available_channels})"
    ))
}

#[derive(Clone, Debug)]
struct DiagnosticsHarnessRunResult {
    passed: bool,
    summary: String,
    receipts: Vec<DiagnosticsHarnessReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticsHarnessScenario {
    SharedLanePack,
    RenderModeHealth,
    SignalRoutingHealth,
    NavigatorProjectionHealth,
    BroadSubsystemSweep,
    PersistenceHealth,
    SecurityIdentityHealth,
}

impl DiagnosticsHarnessScenario {
    fn id(self) -> &'static str {
        match self {
            Self::SharedLanePack => "shared_lane_pack",
            Self::RenderModeHealth => "render_mode_health",
            Self::SignalRoutingHealth => "signal_routing_health",
            Self::NavigatorProjectionHealth => "navigator_projection_health",
            Self::BroadSubsystemSweep => "broad_subsystem_sweep",
            Self::PersistenceHealth => "persistence_health",
            Self::SecurityIdentityHealth => "security_identity_health",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::SharedLanePack => "Shared Lane Pack",
            Self::RenderModeHealth => "Render-Mode Health",
            Self::SignalRoutingHealth => "Signal-Routing Health",
            Self::NavigatorProjectionHealth => "Navigator Projection Health",
            Self::BroadSubsystemSweep => "Broad Subsystem Sweep",
            Self::PersistenceHealth => "Persistence Health",
            Self::SecurityIdentityHealth => "Security / Identity Health",
        }
    }
}

#[derive(Clone, Debug)]
struct DiagnosticsHarnessReceipt {
    scenario_id: &'static str,
    analyzer_id: &'static str,
    signal: AnalyzerSignal,
    summary: String,
}

#[derive(Clone, Debug, Default)]
struct DiagnosticsTestHarnessState {
    run_count: u64,
    last_run: Option<DiagnosticsHarnessRunResult>,
    recent_runs: VecDeque<DiagnosticsHarnessRunResult>,
    selected_scenario: Option<DiagnosticsHarnessScenario>,
}

pub(crate) struct DiagnosticsState {
    active_tab: DiagnosticsTab,
    event_tx: Sender<DiagnosticEvent>,
    event_rx: Receiver<DiagnosticEvent>,
    event_ring: VecDeque<DiagnosticEvent>,
    channel_count_history: HashMap<&'static str, VecDeque<u64>>,
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
    selected_analysis_channel: Option<&'static str>,
    analysis_query: String,
    analysis_only_alerts: bool,
    pinned_analyzer_ids: HashSet<&'static str>,
    pinned_channels: HashSet<&'static str>,
    latency_percentile: LatencyPercentile,
    bottleneck_latency_us: u64,
    export_feedback: Option<String>,
    persistence_health_snapshot: Value,
    history_health_snapshot: Value,
    security_health_snapshot: Value,
    runtime_cache_snapshot: Value,
    tracing_perf_snapshot: Value,
    analyzer_registry: AnalyzerRegistry,
    startup_selfcheck_emitted: bool,
    test_harness_state: DiagnosticsTestHarnessState,
}

impl DiagnosticsState {
    fn register_builtin_analyzers(&mut self) {
        let _ = self.register_analyzer(
            "diagnostics.event_ring_pressure",
            "Event Ring Pressure",
            analyze_event_ring_pressure,
        );
        let _ = self.register_analyzer(
            "startup.selfcheck.structural",
            "Startup Structural Self-Check",
            analyze_startup_structural_selfcheck,
        );
        let _ = self.register_analyzer(
            "tracing.hotpath.latency",
            "Tracing Hotpath Latency",
            analyze_tracing_hotpath_latency,
        );
        let _ = self.register_analyzer(
            "lane.render_mode.health",
            "Lane Receipt: Render-Mode Health",
            analyze_render_mode_health,
        );
        let _ = self.register_analyzer(
            "lane.signal_routing.health",
            "Lane Receipt: Signal-Routing Health",
            analyze_signal_routing_health,
        );
        let _ = self.register_analyzer(
            "lane.navigator_projection.health",
            "Lane Receipt: Navigator Projection Health",
            analyze_navigator_projection_health,
        );
        let _ = self.register_analyzer(
            "storage.persistence.health",
            "Subsystem: Persistence Health",
            analyze_persistence_health,
        );
        let _ = self.register_analyzer(
            "security.identity.health",
            "Subsystem: Security/Identity Health",
            analyze_security_identity_health,
        );
        let _ = self.register_analyzer(
            "diagnostics.registry.health",
            "Diagnostics Registry Health",
            analyze_diagnostics_registry_health,
        );
    }

    fn missing_required_phase_channels() -> Vec<&'static str> {
        let mut present_channels = std::collections::HashSet::new();
        for (descriptor, _config) in diagnostics_registry::list_channel_configs_snapshot() {
            present_channels.insert(descriptor.channel_id);
        }

        let mut missing = Vec::new();
        for descriptor in diagnostics_registry::phase0_required_channels()
            .iter()
            .chain(diagnostics_registry::phase2_required_channels().iter())
            .chain(diagnostics_registry::phase3_required_channels().iter())
        {
            if !present_channels.contains(descriptor.channel_id) {
                missing.push(descriptor.channel_id);
            }
        }

        missing
    }

    fn emit_startup_selfcheck_events(&mut self) {
        if self.startup_selfcheck_emitted {
            return;
        }

        let registered_channel_count = diagnostics_registry::list_channel_configs_snapshot().len();
        let _ = self.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_STARTUP_SELFCHECK_REGISTRIES_LOADED,
            byte_len: registered_channel_count,
        });

        let missing_channels = Self::missing_required_phase_channels();
        let (channel_id, byte_len) = if missing_channels.is_empty() {
            (
                CHANNEL_STARTUP_SELFCHECK_CHANNELS_COMPLETE,
                registered_channel_count,
            )
        } else {
            (
                CHANNEL_STARTUP_SELFCHECK_CHANNELS_INCOMPLETE,
                missing_channels.len(),
            )
        };

        let _ = self.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id,
            byte_len,
        });

        self.startup_selfcheck_emitted = true;
    }

    fn run_harness_scenario(
        &self,
        scenario: DiagnosticsHarnessScenario,
    ) -> DiagnosticsHarnessRunResult {
        let perf = &self.tracing_perf_snapshot;
        let graph = &self.diagnostic_graph;
        let events = &self.event_ring;

        let mut receipts = Vec::new();
        match scenario {
            DiagnosticsHarnessScenario::SharedLanePack => {
                let render = analyze_render_mode_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "lane.render_mode.health",
                    signal: render.signal,
                    summary: render.summary,
                });

                let routing = analyze_signal_routing_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "lane.signal_routing.health",
                    signal: routing.signal,
                    summary: routing.summary,
                });

                let navigator = analyze_navigator_projection_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "lane.navigator_projection.health",
                    signal: navigator.signal,
                    summary: navigator.summary,
                });
            }
            DiagnosticsHarnessScenario::RenderModeHealth => {
                let result = analyze_render_mode_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "lane.render_mode.health",
                    signal: result.signal,
                    summary: result.summary,
                });
            }
            DiagnosticsHarnessScenario::SignalRoutingHealth => {
                let result = analyze_signal_routing_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "lane.signal_routing.health",
                    signal: result.signal,
                    summary: result.summary,
                });
            }
            DiagnosticsHarnessScenario::NavigatorProjectionHealth => {
                let result = analyze_navigator_projection_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "lane.navigator_projection.health",
                    signal: result.signal,
                    summary: result.summary,
                });
            }
            DiagnosticsHarnessScenario::BroadSubsystemSweep => {
                for (analyzer_id, result) in [
                    (
                        "storage.persistence.health",
                        analyze_persistence_health(graph, events, perf),
                    ),
                    (
                        "security.identity.health",
                        analyze_security_identity_health(graph, events, perf),
                    ),
                    (
                        "diagnostics.registry.health",
                        analyze_diagnostics_registry_health(graph, events, perf),
                    ),
                ] {
                    receipts.push(DiagnosticsHarnessReceipt {
                        scenario_id: scenario.id(),
                        analyzer_id,
                        signal: result.signal,
                        summary: result.summary,
                    });
                }
            }
            DiagnosticsHarnessScenario::PersistenceHealth => {
                let result = analyze_persistence_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "storage.persistence.health",
                    signal: result.signal,
                    summary: result.summary,
                });
            }
            DiagnosticsHarnessScenario::SecurityIdentityHealth => {
                let result = analyze_security_identity_health(graph, events, perf);
                receipts.push(DiagnosticsHarnessReceipt {
                    scenario_id: scenario.id(),
                    analyzer_id: "security.identity.health",
                    signal: result.signal,
                    summary: result.summary,
                });
            }
        }

        let alert_count = receipts
            .iter()
            .filter(|receipt| receipt.signal == AnalyzerSignal::Alert)
            .count();
        let summary = format!("{} receipt(s), alert_count={alert_count}", receipts.len(),);

        DiagnosticsHarnessRunResult {
            passed: alert_count == 0,
            summary,
            receipts,
        }
    }

    fn record_harness_run(&mut self, run: DiagnosticsHarnessRunResult) {
        self.test_harness_state.run_count = self.test_harness_state.run_count.saturating_add(1);
        self.test_harness_state.last_run = Some(run.clone());
        self.test_harness_state.recent_runs.push_front(run);
        while self.test_harness_state.recent_runs.len() > 8 {
            self.test_harness_state.recent_runs.pop_back();
        }
    }

    fn analyzer_signal_rollup(&self) -> (usize, usize, usize) {
        let mut quiet = 0usize;
        let mut active = 0usize;
        let mut alert = 0usize;
        for snapshot in self.analyzer_snapshots() {
            match snapshot
                .last_result
                .as_ref()
                .map(|result| result.signal)
                .unwrap_or(AnalyzerSignal::Quiet)
            {
                AnalyzerSignal::Quiet => quiet += 1,
                AnalyzerSignal::Active => active += 1,
                AnalyzerSignal::Alert => alert += 1,
            }
        }
        (quiet, active, alert)
    }

    fn lane_channel_summaries(&self) -> Vec<LaneChannelSummary> {
        let analyzer_signal = |analyzer_id: &'static str| {
            self.analyzer_snapshots()
                .into_iter()
                .find(|snapshot| snapshot.id == analyzer_id)
                .and_then(|snapshot| snapshot.last_result)
                .unwrap_or(AnalyzerResult {
                    signal: AnalyzerSignal::Quiet,
                    summary: "not yet run".to_string(),
                })
        };

        let render_mode_result = analyzer_signal("lane.render_mode.health");
        let signal_routing_result = analyzer_signal("lane.signal_routing.health");
        let navigator_result = analyzer_signal("lane.navigator_projection.health");

        vec![
            LaneChannelSummary {
                lane_id: "render_mode",
                analyzer_id: "lane.render_mode.health",
                label: "Render-Mode Health",
                signal: render_mode_result.signal,
                summary: render_mode_result.summary,
                channel_counts: vec![
                    (
                        CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
                        self.channel_count(CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE),
                    ),
                    (
                        CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
                        self.channel_count(CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY),
                    ),
                    (
                        CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI,
                        self.channel_count(CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI),
                    ),
                    (
                        CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER,
                        self.channel_count(CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER),
                    ),
                    (
                        CHANNEL_VIEWER_FALLBACK_USED,
                        self.channel_count(CHANNEL_VIEWER_FALLBACK_USED),
                    ),
                    (
                        CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
                        self.channel_count(CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE),
                    ),
                ],
            },
            LaneChannelSummary {
                lane_id: "signal_routing",
                analyzer_id: "lane.signal_routing.health",
                label: "Signal-Routing Health",
                signal: signal_routing_result.signal,
                summary: signal_routing_result.summary,
                channel_counts: vec![
                    (
                        CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED,
                        self.channel_count(CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED),
                    ),
                    (
                        CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED,
                        self.channel_count(CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED),
                    ),
                    (
                        CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED,
                        self.channel_count(CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED),
                    ),
                    (
                        CHANNEL_REGISTER_SIGNAL_ROUTING_LAGGED,
                        self.channel_count(CHANNEL_REGISTER_SIGNAL_ROUTING_LAGGED),
                    ),
                    (
                        CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH,
                        self.channel_count(CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH),
                    ),
                ],
            },
            LaneChannelSummary {
                lane_id: "navigator_projection",
                analyzer_id: "lane.navigator_projection.health",
                label: "Navigator Projection Health",
                signal: navigator_result.signal,
                summary: navigator_result.summary,
                channel_counts: vec![
                    (
                        CHANNEL_UX_NAVIGATION_TRANSITION,
                        self.channel_count(CHANNEL_UX_NAVIGATION_TRANSITION),
                    ),
                    (
                        CHANNEL_UX_NAVIGATION_VIOLATION,
                        self.channel_count(CHANNEL_UX_NAVIGATION_VIOLATION),
                    ),
                    (
                        CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH,
                        self.channel_count(CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH),
                    ),
                    (
                        CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK,
                        self.channel_count(CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK),
                    ),
                    (
                        CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION,
                        self.channel_count(CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION),
                    ),
                ],
            },
        ]
    }

    fn top_channel_trends(&self, limit: usize) -> Vec<ChannelTrendSummary> {
        let trend_label = |samples: &[u64]| -> &'static str {
            if samples.len() < 2 {
                return "steady";
            }
            let first = samples.first().copied().unwrap_or(0);
            let last = samples.last().copied().unwrap_or(0);
            if last > first.saturating_add(1_000) && last > ((first as f64) * 1.2) as u64 {
                "rising"
            } else if first > last.saturating_add(1_000) && first > ((last as f64) * 1.2) as u64 {
                "falling"
            } else {
                "steady"
            }
        };

        let mut rows = self
            .diagnostic_graph
            .message_counts
            .iter()
            .map(|(channel_id, count)| {
                let samples = self
                    .diagnostic_graph
                    .message_latency_recent_us
                    .get(channel_id)
                    .map(|values| values.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default();
                let sample_count = self
                    .diagnostic_graph
                    .message_latency_samples
                    .get(channel_id)
                    .copied()
                    .unwrap_or(0);
                let avg_latency_us = if sample_count == 0 {
                    0
                } else {
                    self.diagnostic_graph
                        .message_latency_us
                        .get(channel_id)
                        .copied()
                        .unwrap_or(0)
                        / sample_count
                };
                ChannelTrendSummary {
                    channel_id,
                    message_count: *count,
                    avg_latency_us,
                    trend: trend_label(&samples),
                    recent_samples_us: samples,
                }
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .message_count
                .cmp(&left.message_count)
                .then_with(|| right.avg_latency_us.cmp(&left.avg_latency_us))
        });
        rows.truncate(limit);
        rows
    }

    fn recent_channel_receipts(
        &self,
        channel_id: &'static str,
        limit: usize,
    ) -> Vec<ChannelEventReceipt> {
        let mut receipts = self
            .event_ring
            .iter()
            .rev()
            .filter_map(|event| match event {
                DiagnosticEvent::MessageSent {
                    channel_id: event_channel,
                    byte_len,
                } if *event_channel == channel_id => Some(ChannelEventReceipt {
                    channel_id,
                    direction: "sent",
                    detail: format!("{} bytes", byte_len),
                }),
                DiagnosticEvent::MessageReceived {
                    channel_id: event_channel,
                    latency_us,
                } if *event_channel == channel_id => Some(ChannelEventReceipt {
                    channel_id,
                    direction: "recv",
                    detail: format!("{:.1}ms", *latency_us as f64 / 1000.0),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        receipts.truncate(limit);
        receipts
    }

    fn channel_history_summaries(&self, limit: usize) -> Vec<ChannelHistorySummary> {
        let mut rows = self
            .channel_count_history
            .iter()
            .map(|(channel_id, counts)| ChannelHistorySummary {
                channel_id,
                count_buckets: counts.iter().copied().collect(),
                latency_buckets_us: self
                    .diagnostic_graph
                    .message_latency_recent_us
                    .get(channel_id)
                    .map(|samples| samples.iter().copied().collect())
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            right
                .count_buckets
                .last()
                .copied()
                .unwrap_or(0)
                .cmp(&left.count_buckets.last().copied().unwrap_or(0))
        });
        rows.truncate(limit);
        rows
    }

    fn remediation_hint_for_analyzer(analyzer_id: &str) -> Option<&'static str> {
        match analyzer_id {
            "lane.render_mode.health" => {
                Some("Inspect Compositor and viewer fallback surfaces; verify render-mode receipts and placeholder/fallback reasons.")
            }
            "lane.signal_routing.health" => {
                Some("Inspect routed signal producers/consumers and queue-depth receipts; unresolved failures usually point to missing consumer adoption.")
            }
            "lane.navigator_projection.health" => {
                Some("Check Navigator projection refresh, arrangement-family mapping, and UX navigation violation receipts.")
            }
            "storage.persistence.health" => {
                Some("Inspect persistence startup/open/recover receipts and storage health summaries; failures usually need store/open recovery follow-up.")
            }
            "security.identity.health" => {
                Some("Inspect trust-store, sign/verify, and access-denied receipts; mismatches usually indicate grant or identity authority drift.")
            }
            "diagnostics.registry.health" => {
                Some("Inspect orphan channels and startup self-check output; register missing channels or align owners/contracts.")
            }
            "tracing.hotpath.latency" => {
                Some("Inspect hot channels and recent tracing samples; elevated p95 often indicates a blocked edge between ingress and compositor.")
            }
            "diagnostics.event_ring_pressure" => {
                Some("Reduce noisy channel volume or increase sampling/retention controls before the event ring saturates.")
            }
            _ => None,
        }
    }

    fn analysis_filter_matches(&self, haystack: &str) -> bool {
        let query = self.analysis_query.trim();
        if query.is_empty() {
            return true;
        }
        haystack.to_ascii_lowercase().contains(&query.to_ascii_lowercase())
    }

    fn analysis_snapshot_value(&self) -> Value {
        let analyzers = self
            .analyzer_snapshots()
            .into_iter()
            .map(|snapshot| {
                json!({
                    "id": snapshot.id,
                    "label": snapshot.label,
                    "run_count": snapshot.run_count,
                    "last_result": snapshot.last_result.as_ref().map(|result| json!({
                            "signal": format!("{:?}", result.signal).to_lowercase(),
                            "summary": result.summary,
                        })),
                        "remediation": Self::remediation_hint_for_analyzer(snapshot.id),
                })
            })
            .collect::<Vec<_>>();

        let lane_summaries = self
            .lane_channel_summaries()
            .into_iter()
            .map(|lane| {
                json!({
                    "lane_id": lane.lane_id,
                    "analyzer_id": lane.analyzer_id,
                    "label": lane.label,
                    "signal": format!("{:?}", lane.signal).to_lowercase(),
                    "summary": lane.summary,
                    "channel_counts": lane.channel_counts.into_iter().map(|(channel_id, count)| {
                        json!({
                            "channel_id": channel_id,
                            "count": count,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();

        let channel_trends = self
            .top_channel_trends(8)
            .into_iter()
            .map(|trend| {
                json!({
                    "channel_id": trend.channel_id,
                    "message_count": trend.message_count,
                    "avg_latency_us": trend.avg_latency_us,
                    "trend": trend.trend,
                    "recent_samples_us": trend.recent_samples_us,
                })
            })
            .collect::<Vec<_>>();

        let channel_history = self
            .channel_history_summaries(16)
            .into_iter()
            .map(|history| {
                let sample_values = history.latency_buckets_us.clone();
                let sample_count = sample_values.len() as u64;
                let latest_us = sample_values.last().copied().unwrap_or(0);
                let min_us = sample_values.iter().copied().min().unwrap_or(0);
                let max_us = sample_values.iter().copied().max().unwrap_or(0);
                let avg_us = if sample_count == 0 {
                    0
                } else {
                    sample_values.iter().copied().sum::<u64>() / sample_count
                };
                json!({
                    "channel_id": history.channel_id,
                    "sample_count": sample_count,
                    "latest_us": latest_us,
                    "min_us": min_us,
                    "max_us": max_us,
                    "avg_us": avg_us,
                    "count_buckets": history.count_buckets,
                    "latency_buckets_us": history.latency_buckets_us,
                })
            })
            .collect::<Vec<_>>();

        let harness_recent_runs = self
            .test_harness_state
            .recent_runs
            .iter()
            .map(|run| {
                json!({
                    "passed": run.passed,
                    "summary": run.summary,
                    "receipts": run.receipts.iter().map(|receipt| {
                        json!({
                            "scenario_id": receipt.scenario_id,
                            "analyzer_id": receipt.analyzer_id,
                            "signal": format!("{:?}", receipt.signal).to_lowercase(),
                            "summary": receipt.summary,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();

        let selected_channel_receipts = self
            .selected_analysis_channel
            .map(|channel_id| {
                self.recent_channel_receipts(channel_id, 12)
                    .into_iter()
                    .map(|receipt| {
                        json!({
                            "channel_id": receipt.channel_id,
                            "direction": receipt.direction,
                            "detail": receipt.detail,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        json!({
            "analyzers": analyzers,
            "lane_summaries": lane_summaries,
            "channel_trends": channel_trends,
            "channel_history": channel_history,
            "harness_recent_runs": harness_recent_runs,
            "selected_channel": self.selected_analysis_channel,
            "selected_channel_receipts": selected_channel_receipts,
        })
    }

    fn render_test_harness_scaffold(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Test Harness")
            .default_open(false)
            .show(ui, |ui| {
                ui.small("In-pane harness scaffold for lane-shared diagnostics receipts.");

                let selected = self
                    .test_harness_state
                    .selected_scenario
                    .unwrap_or(DiagnosticsHarnessScenario::SharedLanePack);
                let mut selected_next = selected;
                egui::ComboBox::from_id_salt("diag_harness_scenario")
                    .selected_text(selected.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::SharedLanePack,
                            DiagnosticsHarnessScenario::SharedLanePack.label(),
                        );
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::RenderModeHealth,
                            DiagnosticsHarnessScenario::RenderModeHealth.label(),
                        );
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::SignalRoutingHealth,
                            DiagnosticsHarnessScenario::SignalRoutingHealth.label(),
                        );
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::NavigatorProjectionHealth,
                            DiagnosticsHarnessScenario::NavigatorProjectionHealth.label(),
                        );
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::BroadSubsystemSweep,
                            DiagnosticsHarnessScenario::BroadSubsystemSweep.label(),
                        );
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::PersistenceHealth,
                            DiagnosticsHarnessScenario::PersistenceHealth.label(),
                        );
                        ui.selectable_value(
                            &mut selected_next,
                            DiagnosticsHarnessScenario::SecurityIdentityHealth,
                            DiagnosticsHarnessScenario::SecurityIdentityHealth.label(),
                        );
                    });
                self.test_harness_state.selected_scenario = Some(selected_next);

                if ui.button("Run Lane Harness").clicked() {
                    let run = self.run_harness_scenario(selected_next);
                    self.record_harness_run(run);
                }

                ui.small(format!("Runs: {}", self.test_harness_state.run_count));
                if let Some(last_run) = &self.test_harness_state.last_run {
                    let color = if last_run.passed {
                        egui::Color32::from_rgb(90, 200, 120)
                    } else {
                        egui::Color32::from_rgb(255, 120, 120)
                    };
                    let label = if last_run.passed { "PASS" } else { "FAIL" };
                    ui.colored_label(color, format!("Last run: {label} — {}", last_run.summary));

                    egui::Grid::new("diag_harness_receipts")
                        .num_columns(4)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Scenario");
                            ui.strong("Analyzer");
                            ui.strong("Signal");
                            ui.strong("Receipt");
                            ui.end_row();

                            for receipt in &last_run.receipts {
                                ui.monospace(receipt.scenario_id);
                                ui.monospace(receipt.analyzer_id);
                                let (signal_label, signal_color) = match receipt.signal {
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
                                ui.label(&receipt.summary);
                                ui.end_row();
                            }
                        });
                } else {
                    ui.small("Last run: not executed");
                }

                if !self.test_harness_state.recent_runs.is_empty() {
                    ui.add_space(6.0);
                    ui.small("Recent runs");
                    egui::Grid::new("diag_harness_recent_runs")
                        .num_columns(4)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Scenario");
                            ui.strong("Result");
                            ui.strong("Alerts");
                            ui.strong("Summary");
                            ui.end_row();

                            for run in &self.test_harness_state.recent_runs {
                                let scenario_id = run
                                    .receipts
                                    .first()
                                    .map(|receipt| receipt.scenario_id)
                                    .unwrap_or("unknown");
                                ui.monospace(scenario_id);
                                let (label, color) = if run.passed {
                                    ("PASS", egui::Color32::from_rgb(90, 200, 120))
                                } else {
                                    ("FAIL", egui::Color32::from_rgb(255, 120, 120))
                                };
                                ui.colored_label(color, label);
                                ui.monospace(
                                    run.receipts
                                        .iter()
                                        .filter(|receipt| receipt.signal == AnalyzerSignal::Alert)
                                        .count()
                                        .to_string(),
                                );
                                ui.label(&run.summary);
                                ui.end_row();
                            }
                        });
                }
            });
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
        let restore_verification_fail_count = samples
            .iter()
            .filter(|sample| !sample.restore_verified)
            .count() as u64;
        let chaos_enabled_sample_count =
            samples.iter().filter(|sample| sample.chaos_enabled).count() as u64;
        let latest_sequence = samples.last().map(|sample| sample.sequence);
        let latest_violation_node = samples
            .iter()
            .rev()
            .find(|sample| sample.violation)
            .map(|sample| format!("{:?}", sample.node_key));
        let latest_duration_us = samples.last().map(|sample| sample.duration_us);
        let latest_bridge_mode = samples.last().map(|sample| sample.bridge_mode);
        let bridge_probe_count = self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE);
        let bridge_failed_frame_count =
            self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME);
        let gl_state_violation_count = self.channel_count(CHANNEL_COMPOSITOR_GL_STATE_VIOLATION);
        let pass_order_violation_count =
            self.channel_count(CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION);
        let replay_sample_recorded_count =
            self.channel_count(CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED);
        let replay_artifact_recorded_count =
            self.channel_count(CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED);
        let chaos_probe_count = self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS);
        let chaos_pass_count = self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS);
        let chaos_fail_count = self.channel_count(CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL);
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
            "restore_verification_fail_count": restore_verification_fail_count,
            "chaos_enabled_sample_count": chaos_enabled_sample_count,
            "latest_sequence": latest_sequence,
            "latest_violation_node": latest_violation_node,
            "latest_duration_us": latest_duration_us,
            "latest_bridge_mode": latest_bridge_mode,
            "bridge_probe_count": bridge_probe_count,
            "bridge_failed_frame_count": bridge_failed_frame_count,
            "gl_state_violation_count": gl_state_violation_count,
            "pass_order_violation_count": pass_order_violation_count,
            "replay_sample_recorded_count": replay_sample_recorded_count,
            "replay_artifact_recorded_count": replay_artifact_recorded_count,
            "chaos_probe_count": chaos_probe_count,
            "chaos_pass_count": chaos_pass_count,
            "chaos_fail_count": chaos_fail_count,
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
        let failed_frame_count = samples.iter().filter(|sample| sample.violation).count() as u64;
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
        let restore_verification_fail_count = samples
            .iter()
            .filter(|sample| !sample.restore_verified)
            .count() as u64;
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
        let degradation_gpu_pressure_bytes_total = self
            .diagnostic_graph
            .message_bytes_sent
            .get(CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE)
            .copied()
            .unwrap_or(0);
        let avg_degradation_gpu_pressure_bytes = if degradation_gpu_pressure_count == 0 {
            0
        } else {
            degradation_gpu_pressure_bytes_total / degradation_gpu_pressure_count
        };
        let degradation_placeholder_mode_count =
            self.channel_count(CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE);
        let resource_reuse_context_hit_count =
            self.channel_count(CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT);
        let resource_reuse_context_miss_count =
            self.channel_count(CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS);
        let overlay_batch_sample_count =
            self.channel_count(CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE);
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
            "degradation_gpu_pressure_bytes_total": degradation_gpu_pressure_bytes_total,
            "avg_degradation_gpu_pressure_bytes": avg_degradation_gpu_pressure_bytes,
            "degradation_placeholder_mode_count": degradation_placeholder_mode_count,
            "resource_reuse_context_hit_count": resource_reuse_context_hit_count,
            "resource_reuse_context_miss_count": resource_reuse_context_miss_count,
            "overlay_batch_sample_count": overlay_batch_sample_count,
            "computed_skip_rate_basis_points": computed_skip_rate_basis_points,
            "avg_skip_rate_basis_points": avg_skip_rate_basis_points,
            "avg_overlay_batch_size": avg_overlay_batch_size,
        })
    }

    fn backend_telemetry_summary(&self) -> Value {
        let latest_frame = self.compositor_state.frames.back();
        let mut render_path_counts: HashMap<&'static str, u64> = HashMap::new();
        let mut render_mode_counts: HashMap<&'static str, u64> = HashMap::new();
        let mut mapped_webview_count = 0_u64;
        let mut missing_context_count = 0_u64;
        let mut latest_estimated_visible_content_bytes = 0_u64;
        let mut latest_estimated_composited_content_bytes = 0_u64;
        let mut latest_max_estimated_tile_bytes = 0_u64;
        let mut latest_composited_tile_count = 0_u64;

        if let Some(frame) = latest_frame {
            for tile in &frame.tiles {
                *render_path_counts.entry(tile.render_path_hint).or_insert(0) += 1;
                let render_mode_label = match tile.render_mode {
                    TileRenderMode::CompositedTexture => "composited_texture",
                    TileRenderMode::NativeOverlay => "native_overlay",
                    TileRenderMode::EmbeddedEgui => "embedded_egui",
                    TileRenderMode::Placeholder => "placeholder",
                };
                *render_mode_counts.entry(render_mode_label).or_insert(0) += 1;
                if tile.mapped_webview {
                    mapped_webview_count += 1;
                }
                if !tile.has_context {
                    missing_context_count += 1;
                }
                latest_estimated_visible_content_bytes = latest_estimated_visible_content_bytes
                    .saturating_add(tile.estimated_content_bytes as u64);
                latest_max_estimated_tile_bytes =
                    latest_max_estimated_tile_bytes.max(tile.estimated_content_bytes as u64);
                if tile.render_mode == TileRenderMode::CompositedTexture {
                    latest_composited_tile_count += 1;
                    latest_estimated_composited_content_bytes =
                        latest_estimated_composited_content_bytes
                            .saturating_add(tile.estimated_content_bytes as u64);
                }
            }
        }

        let budget_bytes_per_frame = crate::shell::desktop::workbench::tile_compositor::composited_content_budget_bytes_per_frame() as u64;
        let latest_budget_utilization_basis_points = if budget_bytes_per_frame == 0 {
            0
        } else {
            ((latest_estimated_composited_content_bytes.saturating_mul(10_000))
                / budget_bytes_per_frame)
                .min(10_000)
        };

        json!({
            "latest_frame_sequence": latest_frame.map(|frame| frame.sequence),
            "latest_active_tile_count": latest_frame.map(|frame| frame.active_tile_count).unwrap_or(0),
            "latest_render_path_counts": render_path_counts,
            "latest_render_mode_counts": render_mode_counts,
            "latest_mapped_webview_count": mapped_webview_count,
            "latest_missing_context_count": missing_context_count,
            "budget_bytes_per_frame": budget_bytes_per_frame,
            "latest_estimated_visible_content_bytes": latest_estimated_visible_content_bytes,
            "latest_estimated_composited_content_bytes": latest_estimated_composited_content_bytes,
            "latest_budget_utilization_basis_points": latest_budget_utilization_basis_points,
            "latest_max_estimated_tile_bytes": latest_max_estimated_tile_bytes,
            "latest_composited_tile_count": latest_composited_tile_count,
            "viewer_select_started_count": self.channel_count(CHANNEL_VIEWER_SELECT_STARTED),
            "viewer_select_succeeded_count": self.channel_count(CHANNEL_VIEWER_SELECT_SUCCEEDED),
            "viewer_fallback_used_count": self.channel_count(CHANNEL_VIEWER_FALLBACK_USED),
            "degradation_gpu_pressure_count": self.channel_count(CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE),
            "degradation_placeholder_mode_count": self.channel_count(CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE),
            "content_composed_count": self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED),
            "content_skipped_count": self.channel_count(CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED),
        })
    }

    fn backend_telemetry_report_value(&self) -> Value {
        let summary = self.backend_telemetry_summary();
        let differential = self.compositor_differential_summary();

        json!({
            "schema_name": "graphshell.backend_telemetry_report",
            "schema_version": 1,
            "generated_at_unix_secs": Self::export_timestamp_secs(),
            "publication": {
                "scope": "local-first",
                "verse_ready": true,
                "verse_blob_type": "graphshell.backend_telemetry.v1",
            },
            "summary": summary,
            "compositor_differential": differential,
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
            channel_count_history: HashMap::new(),
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
            selected_analysis_channel: None,
            analysis_query: String::new(),
            analysis_only_alerts: false,
            pinned_analyzer_ids: HashSet::new(),
            pinned_channels: HashSet::new(),
            latency_percentile: LatencyPercentile::P95,
            bottleneck_latency_us: DEFAULT_BOTTLENECK_LATENCY_US,
            export_feedback: None,
            persistence_health_snapshot: json!({}),
            history_health_snapshot: json!({}),
            security_health_snapshot: json!({}),
            runtime_cache_snapshot: json!({}),
            tracing_perf_snapshot: json!({}),
            analyzer_registry: AnalyzerRegistry::default(),
            startup_selfcheck_emitted: false,
            test_harness_state: DiagnosticsTestHarnessState::default(),
        };
        state.register_builtin_analyzers();
        state.emit_startup_selfcheck_events();
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
        self.sync_tracing_perf_snapshot_from_runtime();

        while let Ok(event) = self.event_rx.try_recv() {
            self.aggregate_event(&event);
            self.event_ring.push_back(event);
            while self.event_ring.len() > 512 {
                self.event_ring.pop_front();
            }
        }

        self.snapshot_channel_history_bucket();

        self.analyzer_registry.run_all(
            &self.diagnostic_graph,
            &self.event_ring,
            &self.tracing_perf_snapshot,
        );
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

    fn snapshot_channel_history_bucket(&mut self) {
        const HISTORY_BUCKET_LIMIT: usize = 32;
        let channels = self
            .diagnostic_graph
            .message_counts
            .iter()
            .map(|(channel_id, count)| (*channel_id, *count))
            .collect::<Vec<_>>();
        for (channel_id, count) in channels {
            let history = self.channel_count_history.entry(channel_id).or_default();
            if history.back().copied() != Some(count) {
                history.push_back(count);
                while history.len() > HISTORY_BUCKET_LIMIT {
                    history.pop_front();
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
        self.channel_count_history.clear();
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
                            "pane_id": tile.pane_id,
                            "node_key": format!("{:?}", tile.node_key),
                            "render_mode": format!("{:?}", tile.render_mode),
                            "estimated_content_bytes": tile.estimated_content_bytes,
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

        let signal_trace_entries: Vec<Value> =
            crate::shell::desktop::runtime::registries::phase3_signal_trace_snapshot()
                .iter()
                .map(|entry| {
                    json!({
                        "kind": format!("{:?}", entry.kind),
                        "source": format!("{:?}", entry.source),
                        "causality_stamp": entry.causality_stamp,
                        "observers_notified": entry.observers_notified,
                        "observer_failures": entry.observer_failures,
                    })
                })
                .collect();

        json!({
            "version": 1,
            "generated_at_unix_secs": Self::export_timestamp_secs(),
            "event_ring_len": self.event_ring.len(),
            "analysis": self.analysis_snapshot_value(),
            "persistence_health": self.persistence_health_snapshot.clone(),
            "history_health": self.history_health_snapshot.clone(),
            "security_health": self.security_health_snapshot.clone(),
            "runtime_cache": self.runtime_cache_snapshot.clone(),
            "tracing_perf": self.tracing_perf_snapshot.clone(),
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
            "backend_telemetry": self.backend_telemetry_summary(),
            "backend_telemetry_report": self.backend_telemetry_report_value(),
            "compositor_replay": self.compositor_replay_summary(),
            "compositor_frames": frames,
            "compositor_replay_samples": replay_samples,
            "recent_intents": self.intents.iter().map(|intent| {
                json!({
                    "line": intent.line,
                    "cause": intent.cause.map(|c| format!("{:?}", c)),
                })
            }).collect::<Vec<_>>(),
            "signal_trace": signal_trace_entries,
        })
    }

    pub(crate) fn sync_history_health_snapshot_from_app(&mut self, graph_app: &GraphBrowserApp) {
        let health = graph_app.history_health_summary();
        self.history_health_snapshot = json!({
            "capture_status": format!("{:?}", health.capture_status),
            "recent_traversal_append_failures": health.recent_traversal_append_failures,
            "recent_failure_reason_bucket": health.recent_failure_reason_bucket,
            "last_error": health.last_error,
            "traversal_archive_count": health.traversal_archive_count,
            "dissolved_archive_count": health.dissolved_archive_count,
            "preview_mode_active": health.preview_mode_active,
            "last_preview_isolation_violation": health.last_preview_isolation_violation,
            "replay_in_progress": health.replay_in_progress,
            "replay_cursor": health.replay_cursor,
            "replay_total_steps": health.replay_total_steps,
            "last_return_to_present_result": health.last_return_to_present_result,
            "last_event_unix_ms": health.last_event_unix_ms,
        });
    }

    pub(crate) fn sync_persistence_health_snapshot_from_app(
        &mut self,
        graph_app: &GraphBrowserApp,
    ) {
        let health = graph_app.persistence_health_summary();
        let startup_open_succeeded = self
            .diagnostic_graph
            .message_counts
            .get(CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED)
            .copied()
            .unwrap_or(0);
        let startup_open_failed = self
            .diagnostic_graph
            .message_counts
            .get(CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED)
            .copied()
            .unwrap_or(0);
        let startup_open_timeout = self
            .diagnostic_graph
            .message_counts
            .get(CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT)
            .copied()
            .unwrap_or(0);
        let recovery_succeeded = self
            .diagnostic_graph
            .message_counts
            .get(CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED)
            .copied()
            .unwrap_or(0);
        let recovery_failed = self
            .diagnostic_graph
            .message_counts
            .get(CHANNEL_PERSISTENCE_RECOVER_FAILED)
            .copied()
            .unwrap_or(0);

        let store_status = if startup_open_timeout > 0 {
            "timeout"
        } else if startup_open_failed > 0 {
            "failed"
        } else {
            health.store_status
        };
        let recovery_status = if recovery_succeeded > 0 {
            "succeeded"
        } else if recovery_failed > 0 {
            "failed"
        } else {
            "not_run"
        };

        self.persistence_health_snapshot = json!({
            "store_status": store_status,
            "recovery_status": recovery_status,
            "recovered_graph": health.recovered_graph,
            "startup_open_succeeded_count": startup_open_succeeded,
            "startup_open_failed_count": startup_open_failed,
            "startup_open_timeout_count": startup_open_timeout,
            "recovery_succeeded_count": recovery_succeeded,
            "recovery_failed_count": recovery_failed,
            "snapshot_interval_secs": health.snapshot_interval_secs,
            "last_snapshot_age_secs": health.last_snapshot_age_secs,
            "named_graph_snapshot_count": health.named_graph_snapshot_count,
            "workspace_layout_count": health.workspace_layout_count,
            "traversal_archive_count": health.traversal_archive_count,
            "dissolved_archive_count": health.dissolved_archive_count,
            "workspace_autosave_interval_secs": health.workspace_autosave_interval_secs,
            "workspace_autosave_retention": health.workspace_autosave_retention,
        });
    }

    pub(crate) fn sync_security_health_snapshot_from_runtime(&mut self) {
        let trusted_peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        let workspace_grant_count = trusted_peers
            .iter()
            .map(|peer| peer.workspace_grants.len() as u64)
            .sum::<u64>();
        let signer_backend =
            match crate::shell::desktop::runtime::registries::phase3_nostr_signer_backend_snapshot()
            {
                crate::shell::desktop::runtime::registries::NostrSignerBackendSnapshot::LocalHostKey => {
                    "local_host_key".to_string()
                }
                crate::shell::desktop::runtime::registries::NostrSignerBackendSnapshot::Nip46Delegated {
                    relay_urls,
                    signer_pubkey,
                    connected,
                    ..
                } => format!(
                    "nip46:{}:{}:{}",
                    if connected { "connected" } else { "disconnected" },
                    signer_pubkey,
                    relay_urls.join(",")
                ),
            };

        self.security_health_snapshot = json!({
            "trusted_peer_count": trusted_peers.len(),
            "workspace_grant_count": workspace_grant_count,
            "access_denied_count": self
                .diagnostic_graph
                .message_counts
                .get(CHANNEL_VERSE_SYNC_ACCESS_DENIED)
                .copied()
                .unwrap_or(0),
            "identity_sign_failures": self
                .diagnostic_graph
                .message_counts
                .get(CHANNEL_IDENTITY_SIGN_FAILED)
                .copied()
                .unwrap_or(0),
            "identity_verify_failures": self
                .diagnostic_graph
                .message_counts
                .get(CHANNEL_IDENTITY_VERIFY_FAILED)
                .copied()
                .unwrap_or(0),
            "nostr_signer_backend": signer_backend,
            "nip07_permission_count": crate::shell::desktop::runtime::registries::phase3_nostr_nip07_permission_grants().len(),
            "nostr_subscription_count": crate::shell::desktop::runtime::registries::phase3_nostr_persisted_subscriptions().len(),
        });
    }

    pub(crate) fn sync_runtime_cache_snapshot_from_app(&mut self, graph_app: &GraphBrowserApp) {
        let metrics = graph_app
            .workspace
            .graph_runtime
            .runtime_caches
            .metrics_snapshot();
        self.runtime_cache_snapshot = json!({
            "hits": metrics.hits,
            "misses": metrics.misses,
            "inserts": metrics.inserts,
            "evictions": metrics.evictions,
        });
    }

    pub(crate) fn sync_tracing_perf_snapshot_from_runtime(&mut self) {
        let samples = perf_ring_snapshot();
        let sample_count = samples.len() as u64;

        let mut elapsed_values: Vec<u64> = samples.iter().map(|sample| sample.elapsed_us).collect();
        let total_elapsed_us: u64 = elapsed_values.iter().copied().sum();
        let avg_elapsed_us = if sample_count == 0 {
            0
        } else {
            total_elapsed_us / sample_count
        };
        let max_elapsed_us = elapsed_values.iter().copied().max().unwrap_or(0);
        let p95_elapsed_us = Self::percentile(&mut elapsed_values, 0.95);

        let last_sample_name = samples.last().map(|sample| sample.name.clone());
        let recent_samples: Vec<Value> = samples
            .iter()
            .rev()
            .take(16)
            .map(|sample| {
                json!({
                    "name": sample.name,
                    "elapsed_us": sample.elapsed_us,
                    "captured_at_unix_ms": sample.captured_at_unix_ms,
                })
            })
            .collect();

        self.tracing_perf_snapshot = json!({
            "sample_count": sample_count,
            "avg_elapsed_us": avg_elapsed_us,
            "p95_elapsed_us": p95_elapsed_us,
            "max_elapsed_us": max_elapsed_us,
            "last_sample_name": last_sample_name,
            "recent_samples": recent_samples,
        });
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

    pub(crate) fn highlighted_tile_node(&self) -> Option<NodeKey> {
        self.pinned_node_key.or(self.hovered_node_key)
    }

    pub(crate) fn take_pending_focus_node(&mut self) -> Option<NodeKey> {
        self.pending_focus_node.take()
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
        _tracing_perf_snapshot: &Value,
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

        let startup_selfcheck = snapshots
            .iter()
            .find(|snapshot| snapshot.id == "startup.selfcheck.structural")
            .expect("startup self-check analyzer should be registered");
        assert!(startup_selfcheck.run_count >= 1);
        let startup_result = startup_selfcheck
            .last_result
            .as_ref()
            .expect("startup self-check analyzer should produce result");
        assert_eq!(startup_result.signal, AnalyzerSignal::Active);

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
    fn lane_health_analyzers_are_registered_and_run() {
        let mut state = DiagnosticsState::new();
        state.force_drain_for_tests();

        let snapshots = state.analyzer_snapshots_for_tests();
        for analyzer_id in [
            "lane.render_mode.health",
            "lane.signal_routing.health",
            "lane.navigator_projection.health",
        ] {
            let snapshot = snapshots
                .iter()
                .find(|entry| entry.id == analyzer_id)
                .expect("lane analyzer should be registered");
            assert!(snapshot.run_count >= 1);
            assert!(snapshot.last_result.is_some());
        }
    }

    #[test]
    fn subsystem_health_analyzers_are_registered_and_run() {
        let mut state = DiagnosticsState::new();
        state.force_drain_for_tests();

        let snapshots = state.analyzer_snapshots_for_tests();
        for analyzer_id in [
            "storage.persistence.health",
            "security.identity.health",
            "diagnostics.registry.health",
        ] {
            let snapshot = snapshots
                .iter()
                .find(|entry| entry.id == analyzer_id)
                .expect("subsystem analyzer should be registered");
            assert!(snapshot.run_count >= 1);
            assert!(snapshot.last_result.is_some());
        }
    }

    #[test]
    fn harness_shared_lane_pack_emits_three_receipts() {
        let mut state = DiagnosticsState::new();
        state.force_drain_for_tests();

        let run = state.run_harness_scenario(DiagnosticsHarnessScenario::SharedLanePack);
        assert_eq!(run.receipts.len(), 3);
        assert!(
            run.receipts
                .iter()
                .any(|receipt| receipt.analyzer_id == "lane.render_mode.health")
        );
        assert!(
            run.receipts
                .iter()
                .any(|receipt| receipt.analyzer_id == "lane.signal_routing.health")
        );
        assert!(
            run.receipts
                .iter()
                .any(|receipt| receipt.analyzer_id == "lane.navigator_projection.health")
        );
    }

    #[test]
    fn harness_recent_runs_are_retained_with_cap() {
        let mut state = DiagnosticsState::new();
        state.force_drain_for_tests();

        for _ in 0..10 {
            let run = state.run_harness_scenario(DiagnosticsHarnessScenario::SharedLanePack);
            state.record_harness_run(run);
        }

        assert_eq!(state.test_harness_state.run_count, 10);
        assert_eq!(state.test_harness_state.recent_runs.len(), 8);
        assert!(state.test_harness_state.last_run.is_some());
    }

    #[test]
    fn harness_broad_subsystem_sweep_emits_three_receipts() {
        let mut state = DiagnosticsState::new();
        state.force_drain_for_tests();

        let run = state.run_harness_scenario(DiagnosticsHarnessScenario::BroadSubsystemSweep);
        assert_eq!(run.receipts.len(), 3);
        assert!(
            run.receipts
                .iter()
                .any(|receipt| receipt.analyzer_id == "storage.persistence.health")
        );
        assert!(
            run.receipts
                .iter()
                .any(|receipt| receipt.analyzer_id == "security.identity.health")
        );
        assert!(
            run.receipts
                .iter()
                .any(|receipt| receipt.analyzer_id == "diagnostics.registry.health")
        );
    }

    #[test]
    fn lane_channel_summaries_report_underlying_channel_counts() {
        let mut state = DiagnosticsState::new();
        state
            .diagnostic_graph
            .message_counts
            .insert(CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED, 2);
        state
            .diagnostic_graph
            .message_counts
            .insert(CHANNEL_UX_NAVIGATION_VIOLATION, 1);
        state.force_drain_for_tests();

        let summaries = state.lane_channel_summaries();
        let routing = summaries
            .iter()
            .find(|summary| summary.lane_id == "signal_routing")
            .expect("signal routing lane summary should exist");
        assert_eq!(routing.signal, AnalyzerSignal::Alert);
        assert!(
            routing
                .channel_counts
                .iter()
                .any(|(channel, count)| *channel == CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED
                    && *count == 2)
        );

        let navigator = summaries
            .iter()
            .find(|summary| summary.lane_id == "navigator_projection")
            .expect("navigator lane summary should exist");
        assert_eq!(navigator.signal, AnalyzerSignal::Alert);
        assert!(
            navigator
                .channel_counts
                .iter()
                .any(|(channel, count)| *channel == CHANNEL_UX_NAVIGATION_VIOLATION && *count == 1)
        );
    }

    #[test]
    fn top_channel_trends_identify_rising_latency() {
        let mut state = DiagnosticsState::new();
        state
            .diagnostic_graph
            .message_counts
            .insert("trend.rising", 4);
        state
            .diagnostic_graph
            .message_latency_samples
            .insert("trend.rising", 4);
        state
            .diagnostic_graph
            .message_latency_us
            .insert("trend.rising", 18_000);
        state.diagnostic_graph.message_latency_recent_us.insert(
            "trend.rising",
            VecDeque::from(vec![2_000_u64, 3_000, 5_000, 8_000]),
        );

        let trends = state.top_channel_trends(4);
        let rising = trends
            .iter()
            .find(|trend| trend.channel_id == "trend.rising")
            .expect("rising channel should be present");
        assert_eq!(rising.trend, "rising");
        assert_eq!(rising.avg_latency_us, 4_500);
        assert_eq!(rising.recent_samples_us.len(), 4);
    }

    #[test]
    fn recent_channel_receipts_return_latest_sent_and_received_events() {
        let mut state = DiagnosticsState::new();
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: "diag.channel",
            byte_len: 7,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageReceived {
            channel_id: "diag.channel",
            latency_us: 2_500,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: "other.channel",
            byte_len: 9,
        });
        state.force_drain_for_tests();

        let receipts = state.recent_channel_receipts("diag.channel", 8);
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].direction, "recv");
        assert!(receipts[0].detail.contains("2.5ms"));
        assert_eq!(receipts[1].direction, "sent");
        assert_eq!(receipts[1].detail, "7 bytes");
    }

    #[test]
    fn channel_count_history_records_bucketed_progression() {
        let mut state = DiagnosticsState::new();
        let channel = "history.channel";

        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: channel,
            byte_len: 1,
        });
        state.force_drain_for_tests();

        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: channel,
            byte_len: 1,
        });
        state.force_drain_for_tests();

        let history = state
            .channel_history_summaries(8)
            .into_iter()
            .find(|entry| entry.channel_id == channel)
            .expect("channel history should exist");
        assert_eq!(history.count_buckets, vec![1, 2]);
    }

    #[test]
    fn analysis_snapshot_exports_analyzers_harness_and_channel_history() {
        let mut state = DiagnosticsState::new();
        state
            .diagnostic_graph
            .message_counts
            .insert("analysis.channel", 2);
        state.diagnostic_graph.message_latency_recent_us.insert(
            "analysis.channel",
            VecDeque::from(vec![1_000_u64, 4_000]),
        );
        state
            .diagnostic_graph
            .message_latency_samples
            .insert("analysis.channel", 2);
        state
            .diagnostic_graph
            .message_latency_us
            .insert("analysis.channel", 5_000);
        state.selected_analysis_channel = Some("analysis.channel");

        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: "analysis.channel",
            byte_len: 5,
        });
        let run = state.run_harness_scenario(DiagnosticsHarnessScenario::SharedLanePack);
        state.record_harness_run(run);
        state.force_drain_for_tests();

        let snapshot = state.snapshot_json_value();
        assert!(snapshot["analysis"]["analyzers"].is_array());
        assert!(snapshot["analysis"]["lane_summaries"].is_array());
        assert!(snapshot["analysis"]["channel_trends"].is_array());
        assert!(snapshot["analysis"]["channel_history"].is_array());
        assert!(snapshot["analysis"]["harness_recent_runs"].is_array());
        assert_eq!(
            snapshot["analysis"]["selected_channel"].as_str(),
            Some("analysis.channel")
        );
        assert!(snapshot["analysis"]["selected_channel_receipts"].is_array());
    }

    #[test]
    fn startup_selfcheck_emits_registry_and_channel_contract_records() {
        let mut state = DiagnosticsState::new();
        state.force_drain_for_tests();

        assert!(
            state
                .diagnostic_graph
                .message_counts
                .get(CHANNEL_STARTUP_SELFCHECK_REGISTRIES_LOADED)
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert!(
            state
                .diagnostic_graph
                .message_counts
                .get(CHANNEL_STARTUP_SELFCHECK_CHANNELS_COMPLETE)
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert_eq!(
            state
                .diagnostic_graph
                .message_counts
                .get(CHANNEL_STARTUP_SELFCHECK_CHANNELS_INCOMPLETE)
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn minimal_test_harness_entry_point_reports_pass() {
        let summary = invoke_minimal_test_harness_entry_point()
            .expect("minimal test harness entry point should pass");
        assert!(summary.contains("required_channels="));
        assert!(summary.contains("available_channels="));
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
                pane_id: "pane:test-1".to_string(),
                node_key,
                render_mode: TileRenderMode::CompositedTexture,
                estimated_content_bytes: 8_000,
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
        assert!(snapshot["analysis"].is_object());
        assert!(snapshot["persistence_health"].is_object());
        assert!(snapshot["history_health"].is_object());
        assert!(snapshot["runtime_cache"].is_object());
        assert!(snapshot["tracing_perf"].is_object());
        assert!(snapshot["channels"].is_object());
        assert!(snapshot["spans"].is_object());
        assert!(snapshot["backend_telemetry"].is_object());
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
                pane_id: "pane:test-2".to_string(),
                node_key,
                render_mode: TileRenderMode::CompositedTexture,
                estimated_content_bytes: 16_000,
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
    "channel_keys": Array [
        String("message_bytes_sent"),
        String("message_counts"),
        String("message_latency_recent_us"),
        String("message_latency_samples"),
        String("message_latency_us"),
    ],
    "first_frame_sequence": String("[sequence]"),
    "frame_count": Number(1),
    "generated_at_unix_secs": String("[unix-secs]"),
    "intent_count": Number(1),
    "top_level_keys": Array [
        String("analysis"),
        String("backend_telemetry"),
        String("backend_telemetry_report"),
        String("channels"),
        String("compositor_differential"),
        String("compositor_frames"),
        String("compositor_replay"),
        String("compositor_replay_samples"),
        String("event_ring_len"),
        String("generated_at_unix_secs"),
        String("history_health"),
        String("persistence_health"),
        String("recent_intents"),
        String("runtime_cache"),
        String("security_health"),
        String("signal_trace"),
        String("spans"),
        String("tracing_perf"),
        String("version"),
    ],
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
        assert_eq!(
            payload["measurement_contract"]["sample_count"].as_u64(),
            Some(2)
        );
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
            byte_len: 4_096,
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
            snapshot["compositor_differential"]["degradation_gpu_pressure_bytes_total"].as_u64(),
            Some(4_096)
        );
        assert_eq!(
            snapshot["compositor_differential"]["avg_degradation_gpu_pressure_bytes"].as_u64(),
            Some(4_096)
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
                pane_id: "pane:test-9".to_string(),
                node_key,
                render_mode: TileRenderMode::CompositedTexture,
                estimated_content_bytes: 2_048,
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
    fn snapshot_json_includes_backend_telemetry_summary() {
        let mut state = DiagnosticsState::new();
        let node_key = NodeKey::new(12);
        state.push_frame(CompositorFrameSample {
            sequence: 1,
            active_tile_count: 1,
            focused_node_present: true,
            viewport_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(12.0, 12.0)),
            hierarchy: vec![],
            tiles: vec![CompositorTileSample {
                pane_id: "pane:test-12".to_string(),
                node_key,
                render_mode: TileRenderMode::NativeOverlay,
                estimated_content_bytes: 0,
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(6.0, 6.0)),
                mapped_webview: true,
                has_context: false,
                paint_callback_registered: false,
                render_path_hint: "native-overlay",
            }],
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_VIEWER_SELECT_SUCCEEDED,
            byte_len: 1,
        });
        state.force_drain_for_tests();

        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["backend_telemetry"]["latest_render_path_counts"]["native-overlay"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["backend_telemetry"]["latest_render_mode_counts"]["native_overlay"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["backend_telemetry"]["budget_bytes_per_frame"].as_u64(),
            Some(crate::shell::desktop::workbench::tile_compositor::composited_content_budget_bytes_per_frame() as u64)
        );
        assert_eq!(
            snapshot["backend_telemetry"]["latest_estimated_composited_content_bytes"].as_u64(),
            Some(0)
        );
        assert_eq!(
            snapshot["backend_telemetry"]["viewer_select_succeeded_count"].as_u64(),
            Some(1)
        );
    }

    #[test]
    fn snapshot_json_includes_backend_telemetry_report_schema() {
        let mut state = DiagnosticsState::new();
        let node_key = NodeKey::new(21);
        state.push_frame(CompositorFrameSample {
            sequence: 8,
            active_tile_count: 1,
            focused_node_present: true,
            viewport_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(12.0, 12.0)),
            hierarchy: vec![],
            tiles: vec![CompositorTileSample {
                pane_id: "pane:test-21".to_string(),
                node_key,
                render_mode: TileRenderMode::CompositedTexture,
                estimated_content_bytes: 2_048,
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(6.0, 6.0)),
                mapped_webview: true,
                has_context: true,
                paint_callback_registered: true,
                render_path_hint: "composited",
            }],
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_VIEWER_SELECT_SUCCEEDED,
            byte_len: 1,
        });
        state.force_drain_for_tests();

        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["backend_telemetry_report"]["schema_name"].as_str(),
            Some("graphshell.backend_telemetry_report")
        );
        assert_eq!(
            snapshot["backend_telemetry_report"]["schema_version"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["backend_telemetry_report"]["publication"]["scope"].as_str(),
            Some("local-first")
        );
        assert_eq!(
            snapshot["backend_telemetry_report"]["publication"]["verse_ready"].as_bool(),
            Some(true)
        );
        assert_eq!(
            snapshot["backend_telemetry_report"]["summary"]["latest_frame_sequence"].as_u64(),
            snapshot["backend_telemetry"]["latest_frame_sequence"].as_u64()
        );
        assert_eq!(
            snapshot["backend_telemetry_report"]["summary"]["viewer_select_succeeded_count"]
                .as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["backend_telemetry_report"]["compositor_differential"]["content_composed_count"].as_u64(),
            Some(0)
        );
    }

    #[test]
    fn snapshot_json_includes_gpu_budget_utilization_and_tile_byte_estimates() {
        let mut state = DiagnosticsState::new();
        let node_key = NodeKey::new(17);
        state.push_frame(CompositorFrameSample {
            sequence: 3,
            active_tile_count: 1,
            focused_node_present: true,
            viewport_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(24.0, 24.0)),
            hierarchy: vec![],
            tiles: vec![CompositorTileSample {
                pane_id: "pane:test-17".to_string(),
                node_key,
                render_mode: TileRenderMode::CompositedTexture,
                estimated_content_bytes: 4_096,
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(8.0, 8.0)),
                mapped_webview: true,
                has_context: true,
                paint_callback_registered: true,
                render_path_hint: "composited",
            }],
        });

        state.force_drain_for_tests();
        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["backend_telemetry"]["latest_estimated_visible_content_bytes"].as_u64(),
            Some(4_096)
        );
        assert_eq!(
            snapshot["backend_telemetry"]["latest_estimated_composited_content_bytes"].as_u64(),
            Some(4_096)
        );
        assert_eq!(
            snapshot["backend_telemetry"]["latest_max_estimated_tile_bytes"].as_u64(),
            Some(4_096)
        );
        assert_eq!(
            snapshot["compositor_frames"][0]["tiles"][0]["estimated_content_bytes"].as_u64(),
            Some(4_096)
        );
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
    fn snapshot_json_includes_compositor_contract_health_metrics() {
        let mut state = DiagnosticsState::new();
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_GL_STATE_VIOLATION,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL,
            byte_len: 1,
        });
        let _ = state.event_tx.send(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS,
            byte_len: 1,
        });

        state.force_drain_for_tests();
        let snapshot = state.snapshot_json_value();
        assert_eq!(
            snapshot["compositor_replay"]["gl_state_violation_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["pass_order_violation_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["replay_sample_recorded_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["replay_artifact_recorded_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["chaos_probe_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["chaos_fail_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["compositor_replay"]["chaos_pass_count"].as_u64(),
            Some(1)
        );
    }

    #[test]
    fn snapshot_json_includes_runtime_cache_metrics() {
        let mut state = DiagnosticsState::new();
        let app = GraphBrowserApp::new_for_testing();

        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_suggestions("diag:key".to_string(), vec!["rust".to_string()]);
        let _ = app
            .workspace
            .graph_runtime
            .runtime_caches
            .get_suggestions("diag:key");
        let _ = app
            .workspace
            .graph_runtime
            .runtime_caches
            .get_suggestions("diag:missing");

        state.sync_runtime_cache_snapshot_from_app(&app);
        let snapshot = state.snapshot_json_value();

        assert_eq!(snapshot["runtime_cache"]["inserts"].as_u64(), Some(1));
        assert_eq!(snapshot["runtime_cache"]["hits"].as_u64(), Some(1));
        assert_eq!(snapshot["runtime_cache"]["misses"].as_u64(), Some(1));
        assert_eq!(snapshot["runtime_cache"]["evictions"].as_u64(), Some(0));
    }

    #[test]
    fn security_health_snapshot_reports_trust_and_nostr_runtime_state() {
        let mut state = DiagnosticsState::new();
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        crate::shell::desktop::runtime::registries::phase3_trust_peer(
            crate::mods::native::verse::TrustedPeer {
                node_id: peer_id,
                display_name: "diag-peer".to_string(),
                role: crate::mods::native::verse::PeerRole::Friend,
                added_at: std::time::SystemTime::UNIX_EPOCH,
                last_seen: None,
                workspace_grants: vec![crate::mods::native::verse::WorkspaceGrant {
                    workspace_id: "workspace-diag".to_string(),
                    access: crate::mods::native::verse::AccessLevel::ReadWrite,
                }],
            },
        );
        crate::shell::desktop::runtime::registries::phase3_nostr_use_local_signer();
        crate::shell::desktop::runtime::registries::phase3_nostr_set_nip07_permission(
            "https://example.com",
            "getPublicKey",
            crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Allow,
        )
        .expect("nip07 permission should be stored");
        let _ = crate::shell::desktop::runtime::registries::phase3_nostr_relay_subscribe_for_caller(
            "diag:c1",
            Some("diag-sub"),
            crate::shell::desktop::runtime::registries::nostr_core::NostrFilterSet {
                kinds: vec![1],
                authors: vec![],
                hashtags: vec![],
                relay_urls: vec![],
            },
        );
        state
            .diagnostic_graph
            .message_counts
            .insert(CHANNEL_VERSE_SYNC_ACCESS_DENIED, 3);

        state.sync_security_health_snapshot_from_runtime();
        let snapshot = state.snapshot_json_value();

        assert_eq!(
            snapshot["security_health"]["trusted_peer_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["security_health"]["workspace_grant_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["security_health"]["access_denied_count"].as_u64(),
            Some(3)
        );
        assert_eq!(
            snapshot["security_health"]["nip07_permission_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["security_health"]["nostr_subscription_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["security_health"]["nostr_signer_backend"].as_str(),
            Some("local_host_key")
        );

        crate::shell::desktop::runtime::registries::phase3_revoke_peer(peer_id);
        let _ = crate::shell::desktop::runtime::registries::phase3_restore_nostr_subscriptions(&[]);
        crate::shell::desktop::runtime::registries::phase3_nostr_apply_persisted_nip07_permissions(
            &[],
        )
        .expect("nip07 permissions should clear");
    }

    #[test]
    fn persistence_health_snapshot_reports_store_and_recovery_state() {
        let dir = tempfile::TempDir::new().expect("temp dir should be created");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        app.set_workspace_autosave_retention(2)
            .expect("autosave retention should update");
        app.save_workspace_layout_json("workspace:diag-layout", "{\"root\":null}");
        app.save_named_graph_snapshot("diag-graph")
            .expect("named graph snapshot should save");

        let mut state = DiagnosticsState::new();
        state
            .diagnostic_graph
            .message_counts
            .insert(CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED, 1);
        state
            .diagnostic_graph
            .message_counts
            .insert(CHANNEL_PERSISTENCE_RECOVER_FAILED, 1);

        state.sync_persistence_health_snapshot_from_app(&app);
        let snapshot = state.snapshot_json_value();

        assert_eq!(
            snapshot["persistence_health"]["store_status"].as_str(),
            Some("active")
        );
        assert_eq!(
            snapshot["persistence_health"]["recovery_status"].as_str(),
            Some("failed")
        );
        assert_eq!(
            snapshot["persistence_health"]["workspace_layout_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["persistence_health"]["named_graph_snapshot_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            snapshot["persistence_health"]["workspace_autosave_retention"].as_u64(),
            Some(2)
        );
        assert_eq!(
            snapshot["persistence_health"]["snapshot_interval_secs"].as_u64(),
            Some(crate::services::persistence::DEFAULT_SNAPSHOT_INTERVAL_SECS)
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
                    "contains_intent_pipeline": Bool(true),
                    "contains_percentile_label": Bool(true),
                    "contains_render_pass": Bool(true),
                    "contains_semantic": Bool(true),
                    "contains_servo_runtime": Bool(true),
                    "line_count": Number(6),
                    "starts_with_svg": Bool(true),
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
