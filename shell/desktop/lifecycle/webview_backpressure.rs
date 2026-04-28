/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use graphshell_core::time::PortableInstant;
pub(crate) use graphshell_runtime::NodePaneAttachAttemptMetadata;
use graphshell_runtime::{
    RuntimeWebviewBackpressureMetadataSource, ViewerSurfaceId, WebviewAttachRetryState,
    WebviewCreationBackpressureState, WebviewCreationProbeState, portable_now,
};
use log::warn;
use servo::{OffscreenRenderingContext, WebViewId, WindowRenderingContext};
use url::Url;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause, RuntimeBlockReason, RuntimeEvent};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::registries::infrastructure::mod_loader;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::{EmbedderWindow, WebViewCreationContext};
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_status_sync::{
    renderer_id_from_servo, servo_webview_id_from_renderer,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    self, CHANNEL_MOD_LOAD_FAILED, CHANNEL_VIEWER_SURFACE_ALLOCATE_FAILED,
};
use crate::shell::desktop::workbench::pane_model::PaneId;

// Pragmatic Phase A backpressure:
// Servo webview creation is not fallible in the embedder API, so we infer failure
// from "no semantic signal + no stable live webview" within a timeout window.
//
// 2026-04-26: retry/cooldown numerics live on host-neutral `WebviewAttachRetryState`
// in graphshell-runtime. Probe-timing constants stay shell-side.
// 2026-04-27: per-node state moved to graphshell-runtime as
// `WebviewCreationBackpressureState` using `ViewerSurfaceId` + `PortableInstant`.
// Shell-side adapter functions convert `WebViewId` ↔ `ViewerSurfaceId` through
// the renderer-id registry (`renderer_id_from_servo` / `servo_webview_id_from_renderer`).
const WEBVIEW_CREATION_CONFIRMATION_WINDOW: Duration = Duration::from_secs(2);
const WEBVIEW_CREATION_TIMEOUT: Duration = Duration::from_secs(8);

/// Convert `servo::WebViewId` to the portable [`ViewerSurfaceId`] used in the
/// backpressure probe. The conversion registers the `WebViewId → RendererId`
/// mapping in the renderer-id registry (idempotent) and packs the resulting
/// stable `u64` into a [`ViewerSurfaceId`] via [`ViewerSurfaceId::from_u64`].
fn viewer_surface_id_from_servo_webview(id: WebViewId) -> ViewerSurfaceId {
    ViewerSurfaceId::from_u64(renderer_id_from_servo(id).as_raw())
}

/// Recover `servo::WebViewId` from a [`ViewerSurfaceId`] stored in a probe.
/// Returns `None` when the renderer-id registry no longer has a mapping —
/// e.g. if the webview was closed and its entry was evicted.
fn servo_webview_id_from_viewer_surface(id: ViewerSurfaceId) -> Option<WebViewId> {
    use crate::app::RendererId;
    servo_webview_id_from_renderer(RendererId::from_raw(id.as_u64()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebviewCreationProbeOutcome {
    Confirmed,
    Pending,
    TimedOut,
}

fn cold_restore_url_for_node(
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    node: &crate::graph::Node,
) -> String {
    if node.address.address_kind() == crate::graph::AddressKind::GraphshellClip {
        return graph_app
            .runtime_display_url_for_node(node_key)
            .unwrap_or_else(|| "about:blank".to_string());
    }

    if let Some(url) = node.current_history_url()
        && !url.is_empty()
    {
        return url;
    }
    node.url().to_string()
}

static NODE_PANE_ATTACH_ATTEMPT_METADATA: OnceLock<
    Mutex<HashMap<NodeKey, NodePaneAttachAttemptMetadata>>,
> = OnceLock::new();

fn node_pane_attach_attempt_metadata_cache()
-> &'static Mutex<HashMap<NodeKey, NodePaneAttachAttemptMetadata>> {
    NODE_PANE_ATTACH_ATTEMPT_METADATA.get_or_init(|| Mutex::new(HashMap::new()))
}

struct WebviewCreationBackpressureMetadataSource<'a> {
    states: &'a HashMap<NodeKey, WebviewCreationBackpressureState>,
    now: PortableInstant,
}

impl RuntimeWebviewBackpressureMetadataSource for WebviewCreationBackpressureMetadataSource<'_> {
    fn node_pane_attach_attempt_metadata(&self) -> Vec<(NodeKey, NodePaneAttachAttemptMetadata)> {
        self.states
            .iter()
            .filter_map(|(&node_key, state)| {
                // Probe age and cooldown remaining are already in ms (PortableInstant is ms).
                let pending_attempt_age_ms = state
                    .pending
                    .map(|probe| self.now.0.saturating_sub(probe.started_at.0));
                let cooldown_remaining_ms = state
                    .cooldown_until
                    .and_then(|deadline| deadline.0.checked_sub(self.now.0));
                let metadata = NodePaneAttachAttemptMetadata {
                    retry_count: state.retry.retry_count,
                    pending_attempt_age_ms,
                    cooldown_remaining_ms,
                };
                (!metadata.is_empty()).then_some((node_key, metadata))
            })
            .collect()
    }
}

pub(crate) fn publish_node_pane_attach_attempt_metadata(
    webview_creation_backpressure: &HashMap<NodeKey, WebviewCreationBackpressureState>,
) {
    let source = WebviewCreationBackpressureMetadataSource {
        states: webview_creation_backpressure,
        now: portable_now(),
    };
    let metadata = source
        .node_pane_attach_attempt_metadata()
        .into_iter()
        .collect();

    if let Ok(mut slot) = node_pane_attach_attempt_metadata_cache().lock() {
        *slot = metadata;
    }
}

pub(crate) fn take_node_pane_attach_attempt_metadata()
-> HashMap<NodeKey, NodePaneAttachAttemptMetadata> {
    node_pane_attach_attempt_metadata_cache()
        .lock()
        .map(|mut slot| std::mem::take(&mut *slot))
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn publish_node_pane_attach_attempt_metadata_for_tests(
    metadata: HashMap<NodeKey, NodePaneAttachAttemptMetadata>,
) {
    if let Ok(mut slot) = node_pane_attach_attempt_metadata_cache().lock() {
        *slot = metadata;
    }
}

/// Arm the cooldown on `state`. `now` is a [`PortableInstant`] so the deadline
/// stored in `state.cooldown_until` is host-neutral. Returns the cooldown
/// [`Duration`] so the caller can compute a wall-clock `Instant` deadline for
/// the `MarkRuntimeBlocked` intent (which still uses `std::time::Instant`).
/// The `cooldown_notified` flag is reset to `false`; the caller must set it
/// to `true` after pushing the intent.
fn arm_creation_cooldown(
    state: &mut WebviewCreationBackpressureState,
    now: PortableInstant,
) -> Duration {
    let delay_ms = state.retry.advance_cooldown_step();
    let delay = Duration::from_millis(delay_ms);
    state.cooldown_until = Some(PortableInstant(now.0.saturating_add(delay_ms)));
    state.cooldown_notified = false;
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
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    pane_id: Option<PaneId>,
    node_key: NodeKey,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    #[cfg(feature = "diagnostics")]
    let ensure_started = Instant::now();
    let (Some(node), Some(running_state)) = (
        graph_app.domain_graph().get_node(node_key),
        app_state.as_ref(),
    ) else {
        webview_creation_backpressure.remove(&node_key);
        return;
    };
    if node.lifecycle != NodeLifecycle::Active {
        webview_creation_backpressure.remove(&node_key);
        return;
    }
    let node_url = cold_restore_url_for_node(graph_app, node_key, node);

    if !mod_loader::runtime_has_capability("viewer:webview") {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_MOD_LOAD_FAILED,
            byte_len: node_url.len(),
        });
        return;
    }

    if let Some(existing_webview_id) = graph_app.get_webview_for_node(node_key) {
        if let Some(existing_servo_webview_id) = servo_webview_id_from_renderer(existing_webview_id)
            && window.contains_webview(existing_servo_webview_id)
        {
            if responsive_webviews.contains(&existing_servo_webview_id)
                && let Some(state) = webview_creation_backpressure.get_mut(&node_key)
            {
                state.pending = None;
                state.retry.reset();
                state.cooldown_until = None;
                state.cooldown_notified = false;
            }
            return;
        }
        lifecycle_intents.push(
            RuntimeEvent::UnmapWebview {
                webview_id: existing_webview_id,
            }
            .into(),
        );
    }

    let state = webview_creation_backpressure.entry(node_key).or_default();
    if let Some(cooldown_until) = state.cooldown_until {
        let now = portable_now();
        if now < cooldown_until {
            // Still in cooldown. Push a `MarkRuntimeBlocked` intent once per
            // cooldown window (`cooldown_notified` suppresses redundant pushes
            // on subsequent frames while the same deadline is active).
            if !state.cooldown_notified {
                let remaining_ms = cooldown_until.0.saturating_sub(now.0);
                let retry_at = Instant::now() + Duration::from_millis(remaining_ms);
                lifecycle_intents.push(
                    RuntimeEvent::MarkRuntimeBlocked {
                        key: node_key,
                        reason: RuntimeBlockReason::CreateRetryExhausted,
                        retry_at: Some(retry_at),
                    }
                    .into(),
                );
                state.cooldown_notified = true;
            }
            return;
        }
        state.cooldown_until = None;
        state.cooldown_notified = false;
        state.retry.reset_retry_count();
        lifecycle_intents.push(
            RuntimeEvent::ClearRuntimeBlocked {
                key: node_key,
                cause: LifecycleCause::Restore,
            }
            .into(),
        );
    }
    if state.pending.is_some() {
        return;
    }
    if state.retry.is_retry_exhausted() {
        let delay = arm_creation_cooldown(state, portable_now());
        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: "webview_backpressure.cooldown",
                byte_len: state.retry.retry_count as usize,
            },
        );
        warn!(
            "Pausing webview creation for node {:?} after retry exhaustion; cooldown {:?}",
            node_key, delay
        );
        lifecycle_intents.push(
            RuntimeEvent::MarkRuntimeBlocked {
                key: node_key,
                reason: RuntimeBlockReason::CreateRetryExhausted,
                retry_at: Some(Instant::now() + delay),
            }
            .into(),
        );
        state.cooldown_notified = true;
        state.retry.reset_retry_count();
        return;
    }

    let _ = (base_rendering_context, window_rendering_context);
    if viewer_surface_host
        .allocate_surface(viewer_surfaces, node_key)
        .is_err()
    {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_VIEWER_SURFACE_ALLOCATE_FAILED,
            byte_len: node_url.len(),
        });
        webview_creation_backpressure.remove(&node_key);
        return;
    }
    let Some(render_context) = viewer_surfaces.rendering_context(&node_key) else {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_VIEWER_SURFACE_ALLOCATE_FAILED,
            byte_len: node_url.len(),
        });
        webview_creation_backpressure.remove(&node_key);
        return;
    };
    let pending_create_token = graph_app.take_pending_host_create_token(node_key);
    let webview = if let Some(token) = pending_create_token {
        let Some(request) = running_state.take_pending_create_request(token) else {
            warn!(
                "accepted child create token {:?} for node {:?} was missing at reconcile time",
                token, node_key
            );
            return;
        };

        let webview = request
            .builder(render_context)
            .hidpi_scale_factor(window.platform_window().hidpi_scale_factor())
            .delegate(running_state.clone().webview_delegate())
            .build();
        webview.notify_theme_change(window.platform_window().theme());
        window.add_webview(webview.clone());
        webview
    } else {
        let url = Url::parse(&node_url).unwrap_or_else(|_| Url::parse("about:blank").unwrap());
        window.create_toplevel_webview_with_context(running_state.clone(), url, render_context)
    };
    if let Some(pane_id) = pane_id
        && let Err(error) = registries::phase1_attach_renderer(
            pane_id,
            renderer_id_from_servo(webview.id()),
            Some(node_key),
        )
    {
        warn!(
            "renderer registry rejected pane {:?} -> webview {:?} attachment for node {:?}: {:?}",
            pane_id,
            webview.id(),
            node_key,
            error
        );
        window.close_webview(webview.id());
        return;
    }
    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "webview_backpressure.create_attempt",
            byte_len: 1,
        },
    );
    state.retry.record_attempt();
    state.pending = Some(WebviewCreationProbeState {
        viewer_surface_id: viewer_surface_id_from_servo_webview(webview.id()),
        started_at: portable_now(),
    });
    lifecycle_intents.extend([
        RuntimeEvent::MapWebviewToNode {
            webview_id: renderer_id_from_servo(webview.id()),
            key: node_key,
        }
        .into(),
        lifecycle_intents::promote_node_to_active(node_key, LifecycleCause::Restore).into(),
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
        let Some(node) = graph_app.domain_graph().get_node(node_key) else {
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
            // Convert portable ViewerSurfaceId back to WebViewId for window/registry calls.
            let Some(webview_id) = servo_webview_id_from_viewer_surface(probe.viewer_surface_id)
            else {
                // Renderer mapping lost — treat as if probe never existed.
                state.pending = None;
                continue;
            };
            let contains_webview = window.contains_webview(webview_id);
            let has_responsive_signal = responsive_webviews.contains(&webview_id);
            // Elapsed time from probe start to now, derived from PortableInstant (ms).
            let elapsed_ms = portable_now().0.saturating_sub(probe.started_at.0);
            let elapsed = Duration::from_millis(elapsed_ms);
            match classify_webview_creation_probe(elapsed, contains_webview, has_responsive_signal)
            {
                WebviewCreationProbeOutcome::Confirmed => {
                    #[cfg(feature = "diagnostics")]
                    crate::shell::desktop::runtime::diagnostics::emit_event(
                        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                            channel_id: "webview_backpressure.confirmed",
                            latency_us: elapsed_ms * 1_000,
                        },
                    );
                    state.pending = None;
                    state.retry.reset();
                    state.cooldown_until = None;
                    state.cooldown_notified = false;
                    lifecycle_intents.push(
                        RuntimeEvent::ClearRuntimeBlocked {
                            key: node_key,
                            cause: LifecycleCause::Restore,
                        }
                        .into(),
                    );
                }
                WebviewCreationProbeOutcome::Pending => {}
                WebviewCreationProbeOutcome::TimedOut => {
                    #[cfg(feature = "diagnostics")]
                    crate::shell::desktop::runtime::diagnostics::emit_event(
                        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                            channel_id: "webview_backpressure.timeout",
                            latency_us: elapsed_ms * 1_000,
                        },
                    );
                    if contains_webview {
                        window.close_webview(webview_id);
                    }
                    lifecycle_intents.push(
                        RuntimeEvent::UnmapWebview {
                            webview_id: renderer_id_from_servo(webview_id),
                        }
                        .into(),
                    );
                    state.pending = None;
                    if state.retry.is_retry_exhausted() {
                        let retry_count = state.retry.retry_count;
                        let delay = arm_creation_cooldown(state, portable_now());
                        warn!(
                            "Cooling down node {:?} after {} webview creation retries without confirmation; cooldown {:?}",
                            node_key, retry_count, delay
                        );
                        lifecycle_intents.push(
                            RuntimeEvent::MarkRuntimeBlocked {
                                key: node_key,
                                reason: RuntimeBlockReason::CreateRetryExhausted,
                                retry_at: Some(Instant::now() + delay),
                            }
                            .into(),
                        );
                        state.cooldown_notified = true;
                        state.retry.reset_retry_count();
                    }
                }
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
    use crate::graph::Node;

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

    // Pure cooldown-delay bounds and step-doubling tests live in
    // graphshell-runtime::webview_backpressure now; this test verifies the
    // shell-side adapter layer (PortableInstant deadline arithmetic + state mutation).
    #[test]
    fn test_arm_creation_cooldown_advances_step_and_deadline() {
        let mut state = WebviewCreationBackpressureState::default();
        let now = PortableInstant(10_000); // arbitrary fixed point: 10 s from app start
        let d1 = arm_creation_cooldown(&mut state, now);
        assert_eq!(state.retry.cooldown_step, 1);
        let first_deadline = state.cooldown_until.expect("cooldown deadline set");
        assert!(first_deadline.0 >= now.0 + d1.as_millis() as u64);
        assert!(!state.cooldown_notified);

        let d2 = arm_creation_cooldown(&mut state, now);
        assert_eq!(state.retry.cooldown_step, 2);
        assert!(d2 >= d1);
    }

    fn test_node(url: &str) -> Node {
        Node::test_stub(url)
    }

    #[test]
    fn test_cold_restore_url_for_node_prefers_history_index_entry() {
        let app = GraphBrowserApp::new_for_testing();
        let mut node = test_node("https://fallback.example");
        node.replace_history_state(
            vec![
                "https://example.com/one".to_string(),
                "https://example.com/two".to_string(),
            ],
            1,
        );
        assert_eq!(
            cold_restore_url_for_node(&app, NodeKey::new(0), &node),
            "https://example.com/two".to_string()
        );
    }

    #[test]
    fn test_cold_restore_url_for_node_falls_back_to_node_url_without_history() {
        let app = GraphBrowserApp::new_for_testing();
        let node = test_node("https://fallback.example");
        assert_eq!(
            cold_restore_url_for_node(&app, NodeKey::new(0), &node),
            "https://fallback.example".to_string()
        );
    }
}
