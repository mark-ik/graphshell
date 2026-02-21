/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use servo::Servo;

use crate::window::{EmbedderWindow, EmbedderWindowId, GraphSemanticEvent};

/// Incremental extraction target for embedder-owned runtime state.
///
/// This is intentionally introduced as a scaffold first so we can migrate
/// `RunningAppState` responsibilities in small, behavior-preserving steps.
pub(crate) struct EmbedderCore {
    pub(crate) servo: Servo,
    graph_event_sequence: Arc<AtomicU64>,
    focused_window: RefCell<Option<EmbedderWindowId>>,
    // Keep windows as the last field so windows drop after other runtime references.
    windows: RefCell<HashMap<EmbedderWindowId, Rc<EmbedderWindow>>>,
}

impl EmbedderCore {
    pub(crate) fn new(servo: Servo) -> Self {
        Self {
            servo,
            graph_event_sequence: Arc::new(AtomicU64::new(0)),
            focused_window: RefCell::new(None),
            windows: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn servo(&self) -> &Servo {
        &self.servo
    }

    pub(crate) fn insert_window(&self, window: Rc<EmbedderWindow>) {
        *self.focused_window.borrow_mut() = Some(window.id());
        self.windows.borrow_mut().insert(window.id(), window);
    }

    pub(crate) fn graph_event_sequence_source(&self) -> Arc<AtomicU64> {
        self.graph_event_sequence.clone()
    }

    pub(crate) fn window_count(&self) -> usize {
        self.windows.borrow().len()
    }

    pub(crate) fn focused_window_id(&self) -> Option<EmbedderWindowId> {
        *self.focused_window.borrow()
    }

    pub(crate) fn focused_window(&self) -> Option<Rc<EmbedderWindow>> {
        let focused_id = self.focused_window_id()?;
        self.window(focused_id)
    }

    pub(crate) fn focus_window(&self, window: Rc<EmbedderWindow>) {
        *self.focused_window.borrow_mut() = Some(window.id());
    }

    pub(crate) fn windows<'a>(&'a self) -> Ref<'a, HashMap<EmbedderWindowId, Rc<EmbedderWindow>>> {
        self.windows.borrow()
    }

    pub(crate) fn window(&self, id: EmbedderWindowId) -> Option<Rc<EmbedderWindow>> {
        self.windows.borrow().get(&id).cloned()
    }

    pub(crate) fn maybe_window_for_webview_id(
        &self,
        webview_id: servo::WebViewId,
    ) -> Option<Rc<EmbedderWindow>> {
        for window in self.windows.borrow().values() {
            if window.contains_webview(webview_id) {
                return Some(window.clone());
            }
        }
        None
    }

    pub(crate) fn close_empty_windows(&self, exit_scheduled: bool) {
        self.windows.borrow_mut().retain(|id, window| {
            if !exit_scheduled && !window.should_close() {
                return true;
            }
            if *self.focused_window.borrow() == Some(*id) {
                *self.focused_window.borrow_mut() = None;
            }
            false
        });
    }

    pub(crate) fn drain_window_graph_events(&self) -> Vec<GraphSemanticEvent> {
        let windows: Vec<_> = self.windows.borrow().values().cloned().collect();
        let mut pending_events = Vec::new();
        for window in windows {
            pending_events.extend(window.take_pending_graph_events());
        }
        Self::sort_graph_events(&mut pending_events);
        pending_events
    }

    fn sort_graph_events(events: &mut [GraphSemanticEvent]) {
        events.sort_by_key(|event| event.seq);
    }
}

#[cfg(test)]
mod tests {
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    use crate::window::{GraphSemanticEvent, GraphSemanticEventKind};

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn test_sort_graph_events_orders_by_sequence() {
        let mut events = vec![
            GraphSemanticEvent {
                seq: 9,
                kind: GraphSemanticEventKind::WebViewCrashed {
                    webview_id: test_webview_id(),
                    reason: "x".into(),
                    has_backtrace: false,
                },
            },
            GraphSemanticEvent {
                seq: 2,
                kind: GraphSemanticEventKind::WebViewCrashed {
                    webview_id: test_webview_id(),
                    reason: "y".into(),
                    has_backtrace: false,
                },
            },
            GraphSemanticEvent {
                seq: 5,
                kind: GraphSemanticEventKind::WebViewCrashed {
                    webview_id: test_webview_id(),
                    reason: "z".into(),
                    has_backtrace: false,
                },
            },
        ];

        super::EmbedderCore::sort_graph_events(&mut events);

        assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![2, 5, 9]);
    }
}
