/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
#[cfg(feature = "diagnostics")]
use std::time::Instant;

use crate::app::{RuntimeEvent, WorkbenchIntent};
use crate::shell::desktop::host::window::{GraphSemanticEvent, GraphSemanticEventKind};

pub(crate) fn runtime_events_from_semantic_events(
    events: Vec<GraphSemanticEvent>,
) -> (Vec<RuntimeEvent>, Vec<WorkbenchIntent>) {
    let mut events_out = Vec::with_capacity(events.len());
    let mut workbench_intents = Vec::new();
    for event in events {
        match event.kind {
            GraphSemanticEventKind::UrlChanged {
                webview_id,
                new_url,
            } => {
                #[cfg(feature = "diagnostics")]
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.url_changed",
                        byte_len: 1,
                    },
                );
                events_out.push(RuntimeEvent::WebViewUrlChanged {
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
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.history_changed",
                        byte_len: 1,
                    },
                );
                events_out.push(RuntimeEvent::WebViewHistoryChanged {
                    webview_id,
                    entries,
                    current,
                });
            }
            GraphSemanticEventKind::PageTitleChanged { webview_id, title } => {
                #[cfg(feature = "diagnostics")]
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.title_changed",
                        byte_len: 1,
                    },
                );
                events_out.push(RuntimeEvent::WebViewTitleChanged { webview_id, title });
            }
            GraphSemanticEventKind::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => {
                #[cfg(feature = "diagnostics")]
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.webview_crashed",
                        byte_len: 1,
                    },
                );
                events_out.push(RuntimeEvent::WebViewCrashed {
                    webview_id,
                    reason,
                    has_backtrace,
                });
            }
            GraphSemanticEventKind::HostOpenRequest { request } => {
                #[cfg(feature = "diagnostics")]
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: "semantic.intent.host_open_request",
                        byte_len: 1,
                    },
                );
                events_out.push(RuntimeEvent::HostOpenRequest { request });
            }
            GraphSemanticEventKind::WebDriverWorkbenchIntentRequested { intent } => {
                workbench_intents.push(intent);
            }
        }
    }
    (events_out, workbench_intents)
}

pub(crate) fn runtime_events_and_responsive_from_events(
    events: Vec<GraphSemanticEvent>,
) -> (Vec<RuntimeEvent>, Vec<WorkbenchIntent>, HashSet<servo::WebViewId>) {
    #[cfg(feature = "diagnostics")]
    let ingest_started = Instant::now();
    #[cfg(feature = "diagnostics")]
    let event_count = events.len();
    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "semantic.events_ingest",
            byte_len: event_count,
        },
    );

    let mut responsive_webviews = HashSet::new();

    for event in &events {
        match &event.kind {
            GraphSemanticEventKind::UrlChanged { webview_id, .. }
            | GraphSemanticEventKind::HistoryChanged { webview_id, .. }
            | GraphSemanticEventKind::PageTitleChanged { webview_id, .. } => {
                responsive_webviews.insert(*webview_id);
            }
            GraphSemanticEventKind::WebViewCrashed { .. }
            | GraphSemanticEventKind::HostOpenRequest { .. }
            | GraphSemanticEventKind::WebDriverWorkbenchIntentRequested { .. } => {}
        }
    }

    let (runtime_events, workbench_intents) = runtime_events_from_semantic_events(events);

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_event(
        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "semantic.intents_emitted",
            byte_len: runtime_events.len(),
        },
    );

    #[cfg(feature = "diagnostics")]
    log::trace!(
        "semantic_pipeline ingest_events={} emitted_intents={} responsive_webviews={}",
        event_count,
        runtime_events.len(),
        responsive_webviews.len()
    );

    #[cfg(feature = "diagnostics")]
    {
        let elapsed = ingest_started.elapsed().as_micros() as u64;
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                channel_id: "semantic.events_ingest",
                latency_us: elapsed,
            },
        );
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "semantic_event_pipeline::runtime_events_and_responsive_from_events",
            elapsed,
        );
    }
    (runtime_events, workbench_intents, responsive_webviews)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use proptest::prelude::*;
    use rstest::rstest;
    use servo::WebViewId;
    use tracing_test::traced_test;

    use super::{runtime_events_and_responsive_from_events, runtime_events_from_semantic_events};
    use crate::app::RuntimeEvent;
    use crate::shell::desktop::host::window::{GraphSemanticEvent, GraphSemanticEventKind};

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
        event(GraphSemanticEventKind::WebViewCrashed {
            webview_id: make_webview_id(),
            reason: "boom".to_string(),
            has_backtrace: true,
        }),
        "crash"
    )]
    #[case(
        event(GraphSemanticEventKind::HostOpenRequest {
            request: crate::app::HostOpenRequest {
                url: "servo:newtab".to_string(),
                source: crate::app::OpenSurfaceSource::KeyboardShortcut,
                parent_webview_id: None,
                pending_create_token: None,
            },
        }),
        "host_open"
    )]
    fn test_runtime_events_from_semantic_events_maps_variants(
        #[case] event: GraphSemanticEvent,
        #[case] expected_kind: &str,
    ) {
        let (runtime_events, workbench_intents) = runtime_events_from_semantic_events(vec![event]);
        assert!(workbench_intents.is_empty());
        assert_eq!(runtime_events.len(), 1);
        let kind = match &runtime_events[0] {
            RuntimeEvent::WebViewUrlChanged { .. } => "url",
            RuntimeEvent::WebViewHistoryChanged { .. } => "history",
            RuntimeEvent::WebViewTitleChanged { .. } => "title",
            RuntimeEvent::WebViewCrashed { .. } => "crash",
            RuntimeEvent::HostOpenRequest { .. } => "host_open",
            _ => "other",
        };
        assert_eq!(kind, expected_kind);
    }

    #[derive(Clone, Debug)]
    enum EventSpec {
        UrlChanged,
        HistoryChanged,
        PageTitleChanged,
        HostOpenRequest,
        WebViewCrashed,
    }

    fn event_spec_strategy() -> impl Strategy<Value = EventSpec> {
        prop_oneof![
            Just(EventSpec::UrlChanged),
            Just(EventSpec::HistoryChanged),
            Just(EventSpec::PageTitleChanged),
            Just(EventSpec::HostOpenRequest),
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
            EventSpec::HostOpenRequest => event(GraphSemanticEventKind::HostOpenRequest {
                request: crate::app::HostOpenRequest {
                    url: "https://child.example".to_string(),
                    source: crate::app::OpenSurfaceSource::KeyboardShortcut,
                    parent_webview_id: Some(make_webview_id()),
                    pending_create_token: None,
                },
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
            let expected_responsive = events.iter().fold(HashSet::new(), |mut set, event| {
                match event {
                    GraphSemanticEvent { kind: GraphSemanticEventKind::UrlChanged { webview_id, .. }, .. }
                    | GraphSemanticEvent { kind: GraphSemanticEventKind::HistoryChanged { webview_id, .. }, .. }
                    | GraphSemanticEvent { kind: GraphSemanticEventKind::PageTitleChanged { webview_id, .. }, .. } => {
                        set.insert(*webview_id);
                    },
                    GraphSemanticEvent { kind: GraphSemanticEventKind::HostOpenRequest { .. }, .. } => {},
                    GraphSemanticEvent { kind: GraphSemanticEventKind::WebDriverWorkbenchIntentRequested { .. }, .. } => {},
                    GraphSemanticEvent { kind: GraphSemanticEventKind::WebViewCrashed { .. }, .. } => {},
                }
                set
            });

            let (runtime_events, workbench_intents, responsive) = runtime_events_and_responsive_from_events(events);

            prop_assert_eq!(runtime_events.len(), expected_event_count);
            prop_assert!(workbench_intents.is_empty());
            prop_assert_eq!(responsive, expected_responsive);
        }
    }

    #[test]
    fn test_graph_intents_and_responsive_trace_snapshot() {
        let events = vec![
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: make_webview_id(),
                new_url: "https://pre-existing".to_string(),
            }),
            event(GraphSemanticEventKind::HostOpenRequest {
                request: crate::app::HostOpenRequest {
                    url: "https://child".to_string(),
                    source: crate::app::OpenSurfaceSource::KeyboardShortcut,
                    parent_webview_id: Some(make_webview_id()),
                    pending_create_token: None,
                },
            }),
            event(GraphSemanticEventKind::WebViewCrashed {
                webview_id: make_webview_id(),
                reason: "crash".to_string(),
                has_backtrace: true,
            }),
        ];

        let (runtime_events, workbench_intents, responsive) = runtime_events_and_responsive_from_events(events);
        assert!(workbench_intents.is_empty());
        let intent_kinds = runtime_events
            .iter()
            .map(|intent| match intent {
                RuntimeEvent::WebViewUrlChanged { .. } => "url",
                RuntimeEvent::WebViewHistoryChanged { .. } => "history",
                RuntimeEvent::WebViewTitleChanged { .. } => "title",
                RuntimeEvent::WebViewCrashed { .. } => "crash",
                RuntimeEvent::HostOpenRequest { .. } => "host_open",
                _ => "other",
            })
            .collect::<Vec<_>>();
        let trace = (intent_kinds, responsive.len());

        insta::assert_debug_snapshot!(trace);
    }

    #[test]
    #[traced_test]
    fn test_graph_intents_and_responsive_emits_semantic_pipeline_trace_marker() {
        let events = vec![event(GraphSemanticEventKind::UrlChanged {
            webview_id: make_webview_id(),
            new_url: "https://trace.example".to_string(),
        })];

        let (runtime_events, workbench_intents, responsive) = runtime_events_and_responsive_from_events(events);
        assert!(workbench_intents.is_empty());
        tracing::info!(
            "semantic_pipeline ingest_events={} emitted_intents={} responsive_webviews={}",
            1,
            runtime_events.len(),
            responsive.len()
        );

        assert!(logs_contain("semantic_pipeline ingest_events="));
        assert!(logs_contain("emitted_intents="));
    }
}
