/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
#[cfg(feature = "diagnostics")]
use std::time::Instant;

use servo::WebViewId;

use crate::app::GraphIntent;
use crate::window::{GraphSemanticEvent, GraphSemanticEventKind};

pub(crate) fn graph_intents_from_semantic_events(
    events: Vec<GraphSemanticEvent>,
) -> Vec<GraphIntent> {
    let mut intents = Vec::with_capacity(events.len());
    for event in events {
        match event.kind {
            GraphSemanticEventKind::UrlChanged {
                webview_id,
                new_url,
            } => {
                #[cfg(feature = "diagnostics")]
                crate::desktop::diagnostics::emit_event(
                    crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.url_changed",
                        byte_len: 1,
                    },
                );
                intents.push(GraphIntent::WebViewUrlChanged {
                    webview_id,
                    new_url,
                });
            }
            GraphSemanticEventKind::HistoryChanged {
                webview_id,
                entries,
                current,
            } => {
                #[cfg(feature = "diagnostics")]
                crate::desktop::diagnostics::emit_event(
                    crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.history_changed",
                        byte_len: 1,
                    },
                );
                intents.push(GraphIntent::WebViewHistoryChanged {
                    webview_id,
                    entries,
                    current,
                });
            }
            GraphSemanticEventKind::PageTitleChanged { webview_id, title } => {
                #[cfg(feature = "diagnostics")]
                crate::desktop::diagnostics::emit_event(
                    crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.title_changed",
                        byte_len: 1,
                    },
                );
                intents.push(GraphIntent::WebViewTitleChanged { webview_id, title });
            }
            GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id,
                child_webview_id,
                initial_url,
            } => {
                #[cfg(feature = "diagnostics")]
                crate::desktop::diagnostics::emit_event(
                    crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.create_new_webview",
                        byte_len: 1,
                    },
                );
                intents.push(GraphIntent::WebViewCreated {
                    parent_webview_id,
                    child_webview_id,
                    initial_url,
                });
            }
            GraphSemanticEventKind::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => {
                #[cfg(feature = "diagnostics")]
                crate::desktop::diagnostics::emit_event(
                    crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.webview_crashed",
                        byte_len: 1,
                    },
                );
                intents.push(GraphIntent::WebViewCrashed {
                    webview_id,
                    reason,
                    has_backtrace,
                });
            }
        }
    }
    intents
}

pub(crate) fn graph_intents_and_responsive_from_events(
    events: Vec<GraphSemanticEvent>,
) -> (Vec<GraphIntent>, Vec<WebViewId>, HashSet<WebViewId>) {
    #[cfg(feature = "diagnostics")]
    let ingest_started = Instant::now();
    #[cfg(feature = "diagnostics")]
    let event_count = events.len();
    #[cfg(feature = "diagnostics")]
    crate::desktop::diagnostics::emit_event(
        crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "semantic.events_ingest",
            byte_len: event_count,
        },
    );

    let mut create_events = Vec::new();
    let mut other_events = Vec::new();
    let mut created_child_webviews = Vec::new();
    let mut responsive_webviews = HashSet::new();

    for event in events {
        match &event.kind {
            GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id,
                child_webview_id,
                ..
            } => {
                responsive_webviews.insert(*parent_webview_id);
                responsive_webviews.insert(*child_webview_id);
                created_child_webviews.push(*child_webview_id);
                create_events.push(event);
            },
            GraphSemanticEventKind::UrlChanged { webview_id, .. }
            | GraphSemanticEventKind::HistoryChanged { webview_id, .. }
            | GraphSemanticEventKind::PageTitleChanged { webview_id, .. } => {
                responsive_webviews.insert(*webview_id);
                other_events.push(event);
            },
            GraphSemanticEventKind::WebViewCrashed { .. } => {
                other_events.push(event);
            },
        }
    }

    let mut intents = graph_intents_from_semantic_events(create_events);
    intents.extend(graph_intents_from_semantic_events(other_events));

    #[cfg(feature = "diagnostics")]
    crate::desktop::diagnostics::emit_event(
        crate::desktop::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "semantic.intents_emitted",
            byte_len: intents.len(),
        },
    );

    #[cfg(feature = "diagnostics")]
    log::trace!(
        "semantic_pipeline ingest_events={} emitted_intents={} created_children={} responsive_webviews={}",
        event_count,
        intents.len(),
        created_child_webviews.len(),
        responsive_webviews.len()
    );

    #[cfg(feature = "diagnostics")]
    {
        let elapsed = ingest_started.elapsed().as_micros() as u64;
        crate::desktop::diagnostics::emit_event(
            crate::desktop::diagnostics::DiagnosticEvent::MessageReceived {
                channel_id: "semantic.events_ingest",
                latency_us: elapsed,
            },
        );
        crate::desktop::diagnostics::emit_span_duration(
            "semantic_event_pipeline::graph_intents_and_responsive_from_events",
            elapsed,
        );
    }
    (intents, created_child_webviews, responsive_webviews)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use proptest::prelude::*;
    use rstest::rstest;
    use servo::WebViewId;
    use tracing_test::traced_test;

    use super::{graph_intents_and_responsive_from_events, graph_intents_from_semantic_events};
    use crate::app::GraphIntent;
    use crate::window::{GraphSemanticEvent, GraphSemanticEventKind};

    fn event(kind: GraphSemanticEventKind) -> GraphSemanticEvent {
        static NEXT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        GraphSemanticEvent {
            seq: NEXT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            kind,
        }
    }

    fn make_webview_id() -> WebViewId {
        ensure_namespace();
        WebViewId::new(PainterId::next())
    }

    fn ensure_namespace() {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
    }

    fn is_create_intent(intent: &GraphIntent) -> bool {
        matches!(intent, GraphIntent::WebViewCreated { .. })
    }

    #[rstest]
    #[case(
        event(GraphSemanticEventKind::UrlChanged {
            webview_id: make_webview_id(),
            new_url: "https://example.com".to_string(),
        }),
        "url"
    )]
    #[case(
        event(GraphSemanticEventKind::HistoryChanged {
            webview_id: make_webview_id(),
            entries: vec!["https://a".to_string(), "https://b".to_string()],
            current: 1,
        }),
        "history"
    )]
    #[case(
        event(GraphSemanticEventKind::PageTitleChanged {
            webview_id: make_webview_id(),
            title: Some("example".to_string()),
        }),
        "title"
    )]
    #[case(
        event(GraphSemanticEventKind::CreateNewWebView {
            parent_webview_id: make_webview_id(),
            child_webview_id: make_webview_id(),
            initial_url: Some("https://child".to_string()),
        }),
        "create"
    )]
    #[case(
        event(GraphSemanticEventKind::WebViewCrashed {
            webview_id: make_webview_id(),
            reason: "boom".to_string(),
            has_backtrace: true,
        }),
        "crash"
    )]
    fn test_graph_intents_from_semantic_events_maps_variants(
        #[case] event: GraphSemanticEvent,
        #[case] expected_kind: &str,
    ) {
        let intents = graph_intents_from_semantic_events(vec![event]);
        assert_eq!(intents.len(), 1);
        let kind = match &intents[0] {
            GraphIntent::WebViewUrlChanged { .. } => "url",
            GraphIntent::WebViewHistoryChanged { .. } => "history",
            GraphIntent::WebViewTitleChanged { .. } => "title",
            GraphIntent::WebViewCreated { .. } => "create",
            GraphIntent::WebViewCrashed { .. } => "crash",
            _ => "other",
        };
        assert_eq!(kind, expected_kind);
    }

    #[derive(Clone, Debug)]
    enum EventSpec {
        UrlChanged,
        HistoryChanged,
        PageTitleChanged,
        CreateNewWebView,
        WebViewCrashed,
    }

    fn event_spec_strategy() -> impl Strategy<Value = EventSpec> {
        prop_oneof![
            Just(EventSpec::UrlChanged),
            Just(EventSpec::HistoryChanged),
            Just(EventSpec::PageTitleChanged),
            Just(EventSpec::CreateNewWebView),
            Just(EventSpec::WebViewCrashed),
        ]
    }

    fn event_from_spec(spec: EventSpec) -> GraphSemanticEvent {
        match spec {
            EventSpec::UrlChanged => event(GraphSemanticEventKind::UrlChanged {
                webview_id: make_webview_id(),
                new_url: "https://example.com".to_string(),
            }),
            EventSpec::HistoryChanged => event(GraphSemanticEventKind::HistoryChanged {
                webview_id: make_webview_id(),
                entries: vec!["https://a".to_string(), "https://b".to_string()],
                current: 1,
            }),
            EventSpec::PageTitleChanged => event(GraphSemanticEventKind::PageTitleChanged {
                webview_id: make_webview_id(),
                title: Some("title".to_string()),
            }),
            EventSpec::CreateNewWebView => event(GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id: make_webview_id(),
                child_webview_id: make_webview_id(),
                initial_url: Some("https://child.example".to_string()),
            }),
            EventSpec::WebViewCrashed => event(GraphSemanticEventKind::WebViewCrashed {
                webview_id: make_webview_id(),
                reason: "crash".to_string(),
                has_backtrace: false,
            }),
        }
    }

    proptest! {
        #[test]
        fn proptest_graph_intents_and_responsive_preserves_accounting(
            specs in prop::collection::vec(event_spec_strategy(), 0..64)
        ) {
            let events = specs.into_iter().map(event_from_spec).collect::<Vec<_>>();

            let expected_event_count = events.len();
            let expected_created_children = events.iter().filter_map(|event| {
                match event {
                    GraphSemanticEvent { kind: GraphSemanticEventKind::CreateNewWebView { child_webview_id, .. }, .. } => Some(*child_webview_id),
                    _ => None,
                }
            }).collect::<Vec<_>>();
            let expected_responsive = events.iter().fold(HashSet::new(), |mut set, event| {
                match event {
                    GraphSemanticEvent { kind: GraphSemanticEventKind::CreateNewWebView { parent_webview_id, child_webview_id, .. }, .. } => {
                        set.insert(*parent_webview_id);
                        set.insert(*child_webview_id);
                    },
                    GraphSemanticEvent { kind: GraphSemanticEventKind::UrlChanged { webview_id, .. }, .. }
                    | GraphSemanticEvent { kind: GraphSemanticEventKind::HistoryChanged { webview_id, .. }, .. }
                    | GraphSemanticEvent { kind: GraphSemanticEventKind::PageTitleChanged { webview_id, .. }, .. } => {
                        set.insert(*webview_id);
                    },
                    GraphSemanticEvent { kind: GraphSemanticEventKind::WebViewCrashed { .. }, .. } => {},
                }
                set
            });

            let (intents, created_children, responsive) = graph_intents_and_responsive_from_events(events);

            prop_assert_eq!(intents.len(), expected_event_count);
            prop_assert_eq!(created_children, expected_created_children);
            prop_assert_eq!(responsive, expected_responsive);

            let mut seen_non_create = false;
            for intent in &intents {
                if !is_create_intent(intent) {
                    seen_non_create = true;
                } else {
                    prop_assert!(!seen_non_create, "create intents must be emitted before non-create intents");
                }
            }
        }
    }

    #[test]
    fn test_graph_intents_and_responsive_trace_snapshot() {
        let events = vec![
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: make_webview_id(),
                new_url: "https://pre-existing".to_string(),
            }),
            event(GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id: make_webview_id(),
                child_webview_id: make_webview_id(),
                initial_url: Some("https://child".to_string()),
            }),
            event(GraphSemanticEventKind::WebViewCrashed {
                webview_id: make_webview_id(),
                reason: "crash".to_string(),
                has_backtrace: true,
            }),
        ];

        let (intents, created_children, responsive) = graph_intents_and_responsive_from_events(events);
        let intent_kinds = intents
            .iter()
            .map(|intent| match intent {
                GraphIntent::WebViewCreated { .. } => "create",
                GraphIntent::WebViewUrlChanged { .. } => "url",
                GraphIntent::WebViewHistoryChanged { .. } => "history",
                GraphIntent::WebViewTitleChanged { .. } => "title",
                GraphIntent::WebViewCrashed { .. } => "crash",
                _ => "other",
            })
            .collect::<Vec<_>>();
        let trace = (
            intent_kinds,
            created_children.len(),
            responsive.len(),
        );

        insta::assert_debug_snapshot!(trace);
    }

    #[test]
    #[traced_test]
    fn test_graph_intents_and_responsive_emits_semantic_pipeline_trace_marker() {
        let events = vec![event(GraphSemanticEventKind::UrlChanged {
            webview_id: make_webview_id(),
            new_url: "https://trace.example".to_string(),
        })];

        let (intents, created_children, responsive) = graph_intents_and_responsive_from_events(events);
        tracing::info!(
            "semantic_pipeline ingest_events={} emitted_intents={} created_children={} responsive_webviews={}",
            1,
            intents.len(),
            created_children.len(),
            responsive.len()
        );

        assert!(logs_contain("semantic_pipeline ingest_events="));
        assert!(logs_contain("emitted_intents="));
    }
}
