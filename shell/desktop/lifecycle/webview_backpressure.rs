/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use backon::{BackoffBuilder, ExponentialBuilder};
use log::warn;
use servo::{OffscreenRenderingContext, RenderingContext, WebViewId, WindowRenderingContext};
use url::Url;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause, RuntimeBlockReason};
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_MOD_LOAD_FAILED;
use crate::graph::{NodeKey, NodeLifecycle};
use crate::registries::infrastructure::mod_loader;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;

// Pragmatic Phase A backpressure:
// Servo webview creation is not fallible in the embedder API, so we infer failure
// from "no semantic signal + no stable live webview" within a timeout window.
const WEBVIEW_CREATION_CONFIRMATION_WINDOW: Duration = Duration::from_secs(2);
const WEBVIEW_CREATION_TIMEOUT: Duration = Duration::from_secs(8);
const WEBVIEW_CREATION_MAX_RETRIES: u8 = 3;
const WEBVIEW_CREATION_COOLDOWN_MIN: Duration = Duration::from_secs(1);
const WEBVIEW_CREATION_COOLDOWN_MAX: Duration = Duration::from_secs(30);
const WEBVIEW_CREATION_COOLDOWN_MAX_STEP: usize = 8;

#[derive(Clone, Copy, Debug)]
struct WebviewCreationProbe {
    webview_id: WebViewId,
    started_at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebviewCreationProbeOutcome {
    Confirmed,
    Pending,
    TimedOut,
}

fn cold_restore_url_for_node(node: &crate::graph::Node) -> String {
    if !node.history_entries.is_empty() {
        let idx = node
            .history_index
            .min(node.history_entries.len().saturating_sub(1));
        if let Some(url) = node.history_entries.get(idx)
            && !url.is_empty()
        {
            return url.clone();
        }
    }
    node.url.clone()
}

#[derive(Default, Debug)]
pub(crate) struct WebviewCreationBackpressureState {
    retry_count: u8,
    pending: Option<WebviewCreationProbe>,
    cooldown_until: Option<Instant>,
    cooldown_step: usize,
}

fn creation_cooldown_delay(step: usize) -> Duration {
    let capped_step = step.min(WEBVIEW_CREATION_COOLDOWN_MAX_STEP);
    ExponentialBuilder::default()
        .with_min_delay(WEBVIEW_CREATION_COOLDOWN_MIN)
        .with_max_delay(WEBVIEW_CREATION_COOLDOWN_MAX)
        .with_factor(2.0)
        .with_max_times(capped_step.saturating_add(1))
        .build()
        .nth(capped_step)
        .unwrap_or(WEBVIEW_CREATION_COOLDOWN_MAX)
}

fn arm_creation_cooldown(state: &mut WebviewCreationBackpressureState, now: Instant) -> Duration {
    let delay = creation_cooldown_delay(state.cooldown_step);
    state.cooldown_until = Some(now + delay);
    state.cooldown_step = state.cooldown_step.saturating_add(1);
    delay
}

fn classify_webview_creation_probe(
    elapsed: Duration,
    contains_webview: bool,
    has_responsive_signal: bool,
) -> WebviewCreationProbeOutcome {
    if has_responsive_signal
        || (contains_webview && elapsed >= WEBVIEW_CREATION_CONFIRMATION_WINDOW)
    {
        WebviewCreationProbeOutcome::Confirmed
    } else if elapsed >= WEBVIEW_CREATION_TIMEOUT {
        WebviewCreationProbeOutcome::TimedOut
    } else {
        WebviewCreationProbeOutcome::Pending
    }
}

pub(crate) fn ensure_webview_for_node(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    base_rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    node_key: NodeKey,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    #[cfg(feature = "diagnostics")]
    let ensure_started = Instant::now();
    let (Some(node), Some(running_state)) =
        (graph_app.workspace.graph.get_node(node_key), app_state.as_ref())
    else {
        webview_creation_backpressure.remove(&node_key);
        return;
    };
    if node.lifecycle != NodeLifecycle::Active {
        webview_creation_backpressure.remove(&node_key);
        return;
    }
    let node_url = cold_restore_url_for_node(node);

    if !mod_loader::runtime_has_capability("viewer:webview") {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_MOD_LOAD_FAILED,
            byte_len: node_url.len(),
        });
        return;
    }

    if let Some(existing_webview_id) = graph_app.get_webview_for_node(node_key) {
        if window.contains_webview(existing_webview_id) {
            if responsive_webviews.contains(&existing_webview_id)
                && let Some(state) = webview_creation_backpressure.get_mut(&node_key)
            {
                state.pending = None;
                state.retry_count = 0;
                state.cooldown_until = None;
                state.cooldown_step = 0;
            }
            return;
        }
        lifecycle_intents.push(GraphIntent::UnmapWebview {
            webview_id: existing_webview_id,
        });
    }

    let state = webview_creation_backpressure.entry(node_key).or_default();
    if let Some(deadline) = state.cooldown_until {
        let now = Instant::now();
        if now < deadline {
            if graph_app
                .runtime_block_state_for_node(node_key)
                .map(|state| state.retry_at != Some(deadline))
                .unwrap_or(true)
            {
                lifecycle_intents.push(GraphIntent::MarkRuntimeBlocked {
                    key: node_key,
                    reason: RuntimeBlockReason::CreateRetryExhausted,
                    retry_at: Some(deadline),
                });
            }
            return;
        }
        state.cooldown_until = None;
        state.retry_count = 0;
        lifecycle_intents.push(GraphIntent::ClearRuntimeBlocked {
            key: node_key,
            cause: LifecycleCause::Restore,
        });
    }
    if state.pending.is_some() {
        return;
    }
    if state.retry_count >= WEBVIEW_CREATION_MAX_RETRIES {
        let now = Instant::now();
        let delay = arm_creation_cooldown(state, now);
        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: "webview_backpressure.cooldown",
                byte_len: state.retry_count as usize,
            },
        );
        warn!(
            "Pausing webview creation for node {:?} after retry exhaustion; cooldown {:?}",
            node_key, delay
        );
        lifecycle_intents.push(GraphIntent::MarkRuntimeBlocked {
            key: node_key,
            reason: RuntimeBlockReason::CreateRetryExhausted,
            retry_at: Some(now + delay),
        });
        state.retry_count = 0;
        return;
    }

    let render_context = tile_rendering_contexts
        .entry(node_key)
        .or_insert_with(|| {
            Rc::new(window_rendering_context.offscreen_context(base_rendering_context.size()))
        })
        .clone();
    let url = Url::parse(&node_url).unwrap_or_else(|_| Url::parse("about:blank").unwrap());
    let webview =
        window.create_toplevel_webview_with_context(running_state.clone(), url, render_context);
    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "webview_backpressure.create_attempt",
            byte_len: 1,
        },
    );
    state.retry_count = state.retry_count.saturating_add(1);
    state.pending = Some(WebviewCreationProbe {
        webview_id: webview.id(),
        started_at: Instant::now(),
    });
    lifecycle_intents.extend([
        GraphIntent::MapWebviewToNode {
            webview_id: webview.id(),
            key: node_key,
        },
        lifecycle_intents::promote_node_to_active(node_key, LifecycleCause::Restore),
    ]);
    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "webview_backpressure::ensure_webview_for_node",
        ensure_started.elapsed().as_micros() as u64,
    );
}

pub(crate) fn reconcile_webview_creation_backpressure(
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    #[cfg(feature = "diagnostics")]
    let reconcile_started = Instant::now();
    let tracked_nodes: Vec<NodeKey> = webview_creation_backpressure.keys().copied().collect();
    for node_key in tracked_nodes {
        let Some(node) = graph_app.workspace.graph.get_node(node_key) else {
            webview_creation_backpressure.remove(&node_key);
            continue;
        };
        if node.lifecycle != NodeLifecycle::Active {
            webview_creation_backpressure.remove(&node_key);
            continue;
        }

        if let Some(state) = webview_creation_backpressure.get_mut(&node_key)
            && let Some(probe) = state.pending
        {
            let contains_webview = window.contains_webview(probe.webview_id);
            let has_responsive_signal = responsive_webviews.contains(&probe.webview_id);
            match classify_webview_creation_probe(
                probe.started_at.elapsed(),
                contains_webview,
                has_responsive_signal,
            ) {
                WebviewCreationProbeOutcome::Confirmed => {
                    #[cfg(feature = "diagnostics")]
                    crate::shell::desktop::runtime::diagnostics::emit_event(
                        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                            channel_id: "webview_backpressure.confirmed",
                            latency_us: probe.started_at.elapsed().as_micros() as u64,
                        },
                    );
                    state.pending = None;
                    state.retry_count = 0;
                    state.cooldown_until = None;
                    state.cooldown_step = 0;
                    lifecycle_intents.push(GraphIntent::ClearRuntimeBlocked {
                        key: node_key,
                        cause: LifecycleCause::Restore,
                    });
                },
                WebviewCreationProbeOutcome::Pending => {},
                WebviewCreationProbeOutcome::TimedOut => {
                    #[cfg(feature = "diagnostics")]
                    crate::shell::desktop::runtime::diagnostics::emit_event(
                        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                            channel_id: "webview_backpressure.timeout",
                            latency_us: probe.started_at.elapsed().as_micros() as u64,
                        },
                    );
                    if contains_webview {
                        window.close_webview(probe.webview_id);
                    }
                    lifecycle_intents.push(GraphIntent::UnmapWebview {
                        webview_id: probe.webview_id,
                    });
                    state.pending = None;
                    if state.retry_count >= WEBVIEW_CREATION_MAX_RETRIES {
                        let now = Instant::now();
                        let delay = arm_creation_cooldown(state, now);
                        warn!(
                            "Cooling down node {:?} after {} webview creation retries without confirmation; cooldown {:?}",
                            node_key, state.retry_count, delay
                        );
                        lifecycle_intents.push(GraphIntent::MarkRuntimeBlocked {
                            key: node_key,
                            reason: RuntimeBlockReason::CreateRetryExhausted,
                            retry_at: Some(now + delay),
                        });
                        state.retry_count = 0;
                    }
                },
            }
        }
    }
    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "webview_backpressure::reconcile_webview_creation_backpressure",
        reconcile_started.elapsed().as_micros() as u64,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Node, NodeLifecycle};
    use euclid::default::{Point2D, Vector2D};
    use uuid::Uuid;

    #[test]
    fn test_classify_webview_creation_probe_confirms_on_responsive_signal() {
        let outcome = classify_webview_creation_probe(Duration::from_millis(10), false, true);
        assert_eq!(outcome, WebviewCreationProbeOutcome::Confirmed);
    }

    #[test]
    fn test_classify_webview_creation_probe_confirms_on_stable_live_webview() {
        let outcome = classify_webview_creation_probe(
            WEBVIEW_CREATION_CONFIRMATION_WINDOW + Duration::from_millis(1),
            true,
            false,
        );
        assert_eq!(outcome, WebviewCreationProbeOutcome::Confirmed);
    }

    #[test]
    fn test_classify_webview_creation_probe_times_out_without_confirmation() {
        let outcome = classify_webview_creation_probe(
            WEBVIEW_CREATION_TIMEOUT + Duration::from_millis(1),
            false,
            false,
        );
        assert_eq!(outcome, WebviewCreationProbeOutcome::TimedOut);
    }

    #[test]
    fn test_classify_webview_creation_probe_pending_before_timeout() {
        let outcome = classify_webview_creation_probe(Duration::from_millis(500), false, false);
        assert_eq!(outcome, WebviewCreationProbeOutcome::Pending);
    }

    #[test]
    fn test_creation_cooldown_delay_is_bounded() {
        assert_eq!(creation_cooldown_delay(0), WEBVIEW_CREATION_COOLDOWN_MIN);
        let max_step_delay = creation_cooldown_delay(usize::MAX);
        assert!(max_step_delay >= WEBVIEW_CREATION_COOLDOWN_MIN);
        assert!(max_step_delay <= WEBVIEW_CREATION_COOLDOWN_MAX);
    }

    #[test]
    fn test_arm_creation_cooldown_advances_step_and_deadline() {
        let mut state = WebviewCreationBackpressureState::default();
        let now = Instant::now();
        let d1 = arm_creation_cooldown(&mut state, now);
        assert_eq!(state.cooldown_step, 1);
        let first_deadline = state.cooldown_until.expect("cooldown deadline set");
        assert!(first_deadline >= now + d1);

        let d2 = arm_creation_cooldown(&mut state, now);
        assert_eq!(state.cooldown_step, 2);
        assert!(d2 >= d1);
    }

    fn test_node(url: &str) -> Node {
        Node {
            id: Uuid::new_v4(),
            url: url.to_string(),
            title: url.to_string(),
            position: Point2D::new(0.0, 0.0),
            velocity: Vector2D::new(0.0, 0.0),
            is_pinned: false,
            last_visited: std::time::SystemTime::now(),
            history_entries: Vec::new(),
            history_index: 0,
            thumbnail_png: None,
            thumbnail_width: 0,
            thumbnail_height: 0,
            favicon_rgba: None,
            favicon_width: 0,
            favicon_height: 0,
            session_scroll: None,
            session_form_draft: None,
            lifecycle: NodeLifecycle::Cold,
            mime_hint: None,
            address_kind: crate::model::graph::AddressKind::Http,
        }
    }

    #[test]
    fn test_cold_restore_url_for_node_prefers_history_index_entry() {
        let mut node = test_node("https://fallback.example");
        node.history_entries = vec![
            "https://example.com/one".to_string(),
            "https://example.com/two".to_string(),
        ];
        node.history_index = 1;
        assert_eq!(
            cold_restore_url_for_node(&node),
            "https://example.com/two".to_string()
        );
    }

    #[test]
    fn test_cold_restore_url_for_node_falls_back_to_node_url_without_history() {
        let node = test_node("https://fallback.example");
        assert_eq!(
            cold_restore_url_for_node(&node),
            "https://fallback.example".to_string()
        );
    }
}
