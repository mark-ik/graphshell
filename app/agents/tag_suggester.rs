use std::collections::HashMap;
use std::time::Instant;

use url::Url;

use crate::app::GraphIntent;
use crate::shell::desktop::runtime::control_panel::{IntentSource, QueuedIntent};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_AGENT_INTENT_DROPPED;
use crate::shell::desktop::runtime::registries::agent::{
    Agent, AgentCapability, AgentContext, AgentHandle,
};
use crate::shell::desktop::runtime::registries::signal_routing::{NavigationSignal, SignalKind};

pub(crate) const AGENT_ID_TAG_SUGGESTER: &str = "agent:tag_suggester";

pub(crate) fn instantiate() -> Box<dyn Agent> {
    Box::new(TagSuggesterAgent)
}

pub(crate) struct TagSuggesterAgent;

impl Agent for TagSuggesterAgent {
    fn id(&self) -> &'static str {
        AGENT_ID_TAG_SUGGESTER
    }

    fn display_name(&self) -> &'static str {
        "Tag suggester"
    }

    fn declared_capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::ReadNavigationSignals,
            AgentCapability::SuggestNodeTags,
        ]
    }

    fn spawn(self: Box<Self>, mut context: AgentContext) -> AgentHandle {
        AgentHandle::from_future(async move {
            loop {
                tokio::select! {
                    _ = context.cancel.cancelled() => break,
                    signal = context.signal_rx.recv() => {
                        let Some(signal) = signal else {
                            break;
                        };
                        let SignalKind::Navigation(NavigationSignal::NodeActivated { key, uri, title }) = signal.kind else {
                            continue;
                        };
                        let suggestions =
                            derive_tag_suggestions(context.registries.as_ref(), &uri, &title);
                        if suggestions.is_empty() {
                            continue;
                        }
                        if context
                            .intent_tx
                            .send(QueuedIntent {
                                intent: GraphIntent::SuggestNodeTags { key, suggestions },
                                queued_at: Instant::now(),
                                source: IntentSource::Agent,
                            })
                            .await
                            .is_err()
                        {
                            emit_event(DiagnosticEvent::MessageSent {
                                channel_id: CHANNEL_AGENT_INTENT_DROPPED,
                                byte_len: 1,
                            });
                            break;
                        }
                    }
                }
            }
        })
    }
}

fn derive_tag_suggestions(
    registries: &crate::shell::desktop::runtime::registries::RegistryRuntime,
    uri: &str,
    title: &str,
) -> Vec<String> {
    let mut ranked = HashMap::<String, usize>::new();

    for (query, weight) in suggestion_queries(uri, title) {
        for candidate in registries.suggest_knowledge_tags(&query, 3) {
            *ranked.entry(candidate).or_default() += weight;
        }
    }

    let mut ranked = ranked.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(3);
    ranked.into_iter().map(|(candidate, _)| candidate).collect()
}

fn suggestion_queries(uri: &str, title: &str) -> Vec<(String, usize)> {
    let mut queries = Vec::new();
    let trimmed_title = title.trim();
    if !trimmed_title.is_empty() {
        queries.push((trimmed_title.to_string(), 4));
        queries.extend(
            extract_tokens(trimmed_title)
                .into_iter()
                .map(|token| (token, 2)),
        );
    }

    if let Ok(parsed) = Url::parse(uri) {
        if let Some(host) = parsed.host_str() {
            queries.extend(extract_tokens(host).into_iter().map(|token| (token, 3)));
        }
        queries.extend(
            extract_tokens(parsed.path())
                .into_iter()
                .map(|token| (token, 1)),
        );
    }

    queries
}

fn extract_tokens(input: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "and", "com", "edu", "example", "for", "from", "html", "http", "https", "index", "net",
        "org", "page", "php", "the", "www",
    ];

    let mut tokens = input
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter_map(|part| {
            let token = part.trim().to_ascii_lowercase();
            (!token.is_empty()
                && token.len() >= 3
                && !STOP_WORDS.iter().any(|stop_word| *stop_word == token))
            .then_some(token)
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeKey;
    use crate::shell::desktop::runtime::registries::phase3_shared_runtime;
    use crate::shell::desktop::runtime::registries::signal_routing::{
        NavigationSignal, SignalEnvelope, SignalKind, SignalSource,
    };
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn derive_tag_suggestions_matches_math_title_to_udc_math() {
        let runtime = phase3_shared_runtime();
        let suggestions = derive_tag_suggestions(
            runtime.as_ref(),
            "https://math.example.edu/reference",
            "Mathematics handbook",
        );

        assert!(suggestions.iter().any(|tag| tag == "udc:51"));
    }

    #[tokio::test]
    async fn tag_suggester_agent_emits_suggestion_intent_for_node_activation() {
        let runtime = phase3_shared_runtime();
        let (intent_tx, mut intent_rx) = mpsc::channel(4);
        let signal_bus = runtime.subscribe_all_signals_async();
        let cancel = CancellationToken::new();
        let handle = instantiate().spawn(AgentContext {
            intent_tx,
            signal_rx: signal_bus,
            cancel: cancel.clone(),
            registries: runtime.clone(),
        });

        tokio::spawn(handle.task);

        runtime.publish_signal_for_tests(SignalEnvelope::new(
            SignalKind::Navigation(NavigationSignal::NodeActivated {
                key: NodeKey::new(7),
                uri: "https://math.example.edu/reference".to_string(),
                title: "Mathematics handbook".to_string(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));

        let queued = tokio::time::timeout(std::time::Duration::from_secs(2), intent_rx.recv())
            .await
            .expect("agent should emit a suggestion intent")
            .expect("intent channel should stay open");

        match queued.intent {
            GraphIntent::SuggestNodeTags { key, suggestions } => {
                assert_eq!(key, NodeKey::new(7));
                assert!(suggestions.iter().any(|tag| tag == "udc:51"));
            }
            other => panic!("unexpected queued intent: {other:?}"),
        }

        cancel.cancel();
    }
}
