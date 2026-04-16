/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graphshell semantic event queue emitted from Servo delegate callbacks.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[cfg(all(
    feature = "diagnostics",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::runtime::diagnostics::{self, DiagnosticEvent};
use log::debug;

use super::{WebViewLifecycleEvent, WebViewLifecycleEventKind};

pub(super) struct WindowGraphEventQueue {
    pub(super) pending_events: RefCell<Vec<WebViewLifecycleEvent>>,
    pub(super) sequence: Arc<AtomicU64>,
    pub(super) trace_enabled: bool,
    pub(super) trace_started_at: Instant,
    pub(super) trace_drains: Cell<u64>,
}

impl WindowGraphEventQueue {
    pub(super) fn new(sequence: Arc<AtomicU64>) -> Self {
        Self {
            pending_events: Default::default(),
            sequence,
            trace_enabled: std::env::var_os("GRAPHSHELL_TRACE_DELEGATE_EVENTS").is_some(),
            trace_started_at: Instant::now(),
            trace_drains: Cell::new(0),
        }
    }

    pub(super) fn enqueue(&self, kind: WebViewLifecycleEventKind) {
        let event = self.new_event(kind);
        self.trace_event(&event);
        self.pending_events.borrow_mut().push(event);
    }

    pub(super) fn take_pending(&self) -> Vec<WebViewLifecycleEvent> {
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        let drain_started = Instant::now();

        let events = std::mem::take(&mut *self.pending_events.borrow_mut());

        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        {
            diagnostics::emit_event(DiagnosticEvent::MessageReceived {
                channel_id: "window.graph_event.drain",
                latency_us: drain_started.elapsed().as_micros() as u64,
            });
            diagnostics::emit_event(DiagnosticEvent::MessageReceived {
                channel_id: "servo.graph_event.drain",
                latency_us: drain_started.elapsed().as_micros() as u64,
            });
            diagnostics::emit_event(DiagnosticEvent::MessageSent {
                channel_id: "window.graph_event.drain_count",
                byte_len: events.len(),
            });
            diagnostics::emit_event(DiagnosticEvent::MessageSent {
                channel_id: "servo.graph_event.drain_count",
                byte_len: events.len(),
            });
        }

        if self.trace_enabled {
            let drain_id = self.trace_drains.get() + 1;
            self.trace_drains.set(drain_id);
            let elapsed_ms = self.trace_started_at.elapsed().as_millis();
            debug!(
                "graph_event_trace drain={} t_ms={} count={}",
                drain_id,
                elapsed_ms,
                events.len()
            );
        }

        events
    }

    #[cfg(test)]
    pub(super) fn enqueue_for_test(&self, kind: WebViewLifecycleEventKind) {
        self.enqueue(kind);
    }

    fn new_event(&self, kind: WebViewLifecycleEventKind) -> WebViewLifecycleEvent {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        WebViewLifecycleEvent { seq, kind }
    }

    fn trace_event(&self, event: &WebViewLifecycleEvent) {
        if !self.trace_enabled {
            return;
        }

        let elapsed_ms = self.trace_started_at.elapsed().as_millis();
        match &event.kind {
            WebViewLifecycleEventKind::UrlChanged {
                webview_id,
                new_url,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=url_changed webview={:?} url={}",
                    event.seq, elapsed_ms, webview_id, new_url
                );
            }
            WebViewLifecycleEventKind::HistoryChanged {
                webview_id,
                entries,
                current,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=history_changed webview={:?} entries_len={} current={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    entries.len(),
                    current
                );
            }
            WebViewLifecycleEventKind::PageTitleChanged { webview_id, title } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=title_changed webview={:?} title_present={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    title.as_deref().is_some_and(|value| !value.is_empty())
                );
            }
            WebViewLifecycleEventKind::HostOpenRequest { request } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=host_open_request url={} source={:?} parent={:?}",
                    event.seq, elapsed_ms, request.url, request.source, request.parent_webview_id
                );
            }
            WebViewLifecycleEventKind::WebDriverWorkbenchIntentRequested { intent } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=webdriver_workbench_intent intent={intent:?}",
                    event.seq, elapsed_ms
                );
            }
            WebViewLifecycleEventKind::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=webview_crashed webview={:?} reason_len={} has_backtrace={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    reason.len(),
                    has_backtrace
                );
            }
        }
    }
}
