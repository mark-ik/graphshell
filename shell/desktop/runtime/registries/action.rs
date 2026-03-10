use std::collections::HashMap;

use crate::app::{GraphBrowserApp, GraphIntent, GraphMutation, LifecycleCause, RuntimeEvent};
use crate::graph::NodeKey;
use crate::services::search::fuzzy_match_node_keys;
use euclid::default::Point2D;

pub(crate) const ACTION_OMNIBOX_NODE_SEARCH: &str = "action.omnibox_node_search";
pub(crate) const ACTION_GRAPH_VIEW_SUBMIT: &str = "action.graph_view_submit";
pub(crate) const ACTION_DETAIL_VIEW_SUBMIT: &str = "action.detail_view_submit";

// Verse sync actions (Step 5.3)
pub(crate) const ACTION_VERSE_PAIR_DEVICE: &str = "action.verse.pair_device";
pub(crate) const ACTION_VERSE_SYNC_NOW: &str = "action.verse.sync_now";
pub(crate) const ACTION_VERSE_SHARE_WORKSPACE: &str = "action.verse.share_workspace";
pub(crate) const ACTION_VERSE_FORGET_DEVICE: &str = "action.verse.forget_device";

#[derive(Debug, Clone)]
pub(crate) enum ActionPayload {
    OmniboxNodeSearch {
        query: String,
    },
    GraphViewSubmit {
        input: String,
    },
    DetailViewSubmit {
        normalized_url: String,
        focused_node: Option<NodeKey>,
    },
    // Verse sync actions (Step 5.3)
    VersePairDevice {
        mode: PairingMode,
    },
    VerseSyncNow,
    VerseShareWorkspace {
        workspace_id: String,
    },
    VerseForgetDevice {
        node_id: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum PairingMode {
    /// Show our pairing code for others to enter
    ShowCode,
    /// Enter someone else's code to pair with them
    EnterCode { code: String },
    /// Pair with a discovered mDNS peer
    LocalPeer { node_id: String },
}

type ActionHandler = fn(&GraphBrowserApp, &ActionPayload) -> ActionOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionFailureKind {
    UnknownAction,
    InvalidPayload,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionFailure {
    pub(crate) kind: ActionFailureKind,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) enum ActionOutcome {
    Intents(Vec<GraphIntent>),
    Failure(ActionFailure),
}

impl ActionOutcome {
    pub(crate) fn succeeded(&self) -> bool {
        matches!(self, Self::Intents(_))
    }

    pub(crate) fn intent_len(&self) -> usize {
        match self {
            Self::Intents(intents) => intents.len(),
            Self::Failure(_) => 0,
        }
    }

    pub(crate) fn into_intents(self) -> Vec<GraphIntent> {
        match self {
            Self::Intents(intents) => intents,
            Self::Failure(_) => Vec::new(),
        }
    }
}

pub(crate) struct ActionRegistry {
    handlers: HashMap<String, ActionHandler>,
}

impl ActionRegistry {
    pub(crate) fn register(&mut self, action_id: &str, handler: ActionHandler) {
        self.handlers
            .insert(action_id.to_ascii_lowercase(), handler);
    }

    pub(crate) fn execute(
        &self,
        action_id: &str,
        app: &GraphBrowserApp,
        payload: ActionPayload,
    ) -> ActionOutcome {
        let normalized_action_id = action_id.to_ascii_lowercase();
        if let Some(handler) = self.handlers.get(&normalized_action_id) {
            return handler(app, &payload);
        }

        ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::UnknownAction,
            reason: format!("unknown action: {normalized_action_id}"),
        })
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        registry.register(ACTION_DETAIL_VIEW_SUBMIT, execute_detail_view_submit_action);
        registry.register(ACTION_GRAPH_VIEW_SUBMIT, execute_graph_view_submit_action);
        registry.register(
            ACTION_OMNIBOX_NODE_SEARCH,
            execute_omnibox_node_search_action,
        );

        // Verse sync actions (Step 5.3)
        registry.register(ACTION_VERSE_PAIR_DEVICE, execute_verse_pair_device_action);
        registry.register(ACTION_VERSE_SYNC_NOW, execute_verse_sync_now_action);
        registry.register(
            ACTION_VERSE_SHARE_WORKSPACE,
            execute_verse_share_workspace_action,
        );
        registry.register(
            ACTION_VERSE_FORGET_DEVICE,
            execute_verse_forget_device_action,
        );

        registry
    }
}

fn execute_graph_view_submit_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphViewSubmit { input } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph_view_submit requires GraphViewSubmit payload".to_string(),
        });
    };

    let input = input.trim();
    if input.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "graph_view_submit rejected empty input".to_string(),
        });
    }

    if let Some(selected_node) = app.get_single_selected_node() {
        ActionOutcome::Intents(vec![
            GraphMutation::SetNodeUrl {
                key: selected_node,
                new_url: input.to_string(),
            }
            .into(),
        ])
    } else {
        let position = new_node_position_for_context(app, app.focused_selection().primary());
        ActionOutcome::Intents(vec![
            GraphMutation::CreateNodeAtUrl {
                url: input.to_string(),
                position,
            }
            .into(),
        ])
    }
}

fn execute_detail_view_submit_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::DetailViewSubmit {
        normalized_url,
        focused_node,
    } = payload
    else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "detail_view_submit requires DetailViewSubmit payload".to_string(),
        });
    };

    if normalized_url.trim().is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "detail_view_submit rejected empty url".to_string(),
        });
    }

    if let Some(node_key) = focused_node {
        return ActionOutcome::Intents(vec![
            GraphMutation::SetNodeUrl {
                key: *node_key,
                new_url: normalized_url.clone(),
            }
            .into(),
            RuntimeEvent::PromoteNodeToActive {
                key: *node_key,
                cause: LifecycleCause::Restore,
            }
            .into(),
        ]);
    }

    ActionOutcome::Intents(vec![
        GraphMutation::CreateNodeAtUrl {
            url: normalized_url.clone(),
            position: new_node_position_for_context(app, app.focused_selection().primary()),
        }
        .into(),
    ])
}

fn graph_centroid_or_default(app: &GraphBrowserApp) -> Point2D<f32> {
    app.workspace
        .domain
        .graph
        .projected_centroid()
        .unwrap_or_else(|| Point2D::new(400.0, 300.0))
}

fn new_node_position_for_context(app: &GraphBrowserApp, anchor: Option<NodeKey>) -> Point2D<f32> {
    let base = anchor
        .and_then(|key| app.domain_graph().node_projected_position(key))
        .unwrap_or_else(|| graph_centroid_or_default(app));
    let n = app.domain_graph().node_count() as f32;
    let angle = n * std::f32::consts::FRAC_PI_4;
    let radius = 90.0;
    Point2D::new(base.x + radius * angle.cos(), base.y + radius * angle.sin())
}

fn execute_omnibox_node_search_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::OmniboxNodeSearch { query } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "omnibox_node_search requires OmniboxNodeSearch payload".to_string(),
        });
    };

    let matched_keys = fuzzy_match_node_keys(app.domain_graph(), query);
    if let Some(key) = matched_keys.first() {
        return ActionOutcome::Intents(vec![GraphIntent::SelectNode {
            key: *key,
            multi_select: false,
        }]);
    }
    ActionOutcome::Failure(ActionFailure {
        kind: ActionFailureKind::Rejected,
        reason: format!("omnibox_node_search found no match for '{query}'"),
    })
}

// ===== Verse Sync Action Handlers (Step 5.3) =====

fn execute_verse_pair_device_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::VersePairDevice { mode } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "verse_pair_device requires VersePairDevice payload".to_string(),
        });
    };

    match mode {
        PairingMode::ShowCode => ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_pair_device show-code is handled by the UI surface".to_string(),
        }),
        PairingMode::EnterCode { code } => match crate::mods::native::verse::decode_pairing_code(code)
        {
            Ok(node_id) => ActionOutcome::Intents(vec![GraphIntent::TrustPeer {
                peer_id: node_id.to_string(),
                display_name: format!("Paired {}", &node_id.to_string()[..8]),
            }]),
            Err(error) => ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::Rejected,
                reason: format!("pairing code decode failed: {error}"),
            }),
        },
        PairingMode::LocalPeer { node_id } => match node_id.parse::<iroh::NodeId>() {
            Ok(parsed_node_id) => ActionOutcome::Intents(vec![GraphIntent::TrustPeer {
                peer_id: parsed_node_id.to_string(),
                display_name: format!("Local {}", &parsed_node_id.to_string()[..8]),
            }]),
            Err(error) => ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::Rejected,
                reason: format!("invalid local peer id '{node_id}': {error}"),
            }),
        },
    }
}

fn execute_verse_sync_now_action(
    _app: &GraphBrowserApp,
    _payload: &ActionPayload,
) -> ActionOutcome {
    ActionOutcome::Intents(vec![crate::app::RuntimeEvent::SyncNow.into()])
}

fn execute_verse_share_workspace_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::VerseShareWorkspace { workspace_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "verse_share_workspace requires VerseShareWorkspace payload".to_string(),
        });
    };

    if workspace_id.trim().is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_share_workspace rejected empty workspace id".to_string(),
        });
    }

    let peers = crate::mods::native::verse::get_trusted_peers();
    if peers.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_share_workspace has no trusted peers to share with".to_string(),
        });
    }

    ActionOutcome::Intents(
        peers
            .into_iter()
            .map(|peer| GraphIntent::GrantWorkspaceAccess {
                peer_id: peer.node_id.to_string(),
                workspace_id: workspace_id.clone(),
            })
            .collect(),
    )
}

fn execute_verse_forget_device_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::VerseForgetDevice { node_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "verse_forget_device requires VerseForgetDevice payload".to_string(),
        });
    };

    if node_id.trim().is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_forget_device rejected empty peer id".to_string(),
        });
    }

    ActionOutcome::Intents(vec![
        GraphMutation::ForgetDevice {
            peer_id: node_id.clone(),
        }
        .into(),
    ])
}

// ===== Core Action Implementations (original) =====

#[cfg(test)]
mod tests {
    use super::*;
    use euclid::default::Point2D;

    #[test]
    fn action_registry_executes_omnibox_node_search() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example Handle".into();
        }

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_OMNIBOX_NODE_SEARCH,
            &app,
            ActionPayload::OmniboxNodeSearch {
                query: "example handle".to_string(),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SelectNode { key: selected, .. }) if *selected == key
        ));
    }

    #[test]
    fn action_registry_returns_failed_outcome_for_unknown_action_id() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();
        let execution = registry.execute(
            "action.unknown",
            &app,
            ActionPayload::OmniboxNodeSearch {
                query: "payload".to_string(),
            },
        );

        assert!(matches!(
            execution,
            ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::UnknownAction,
                ..
            })
        ));
    }

    #[test]
    fn action_registry_executes_graph_view_submit_action() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_GRAPH_VIEW_SUBMIT,
            &app,
            ActionPayload::GraphViewSubmit {
                input: "https://next.com".to_string(),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://next.com"
        ));
    }

    #[test]
    fn action_registry_executes_detail_view_submit_action_for_focused_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_DETAIL_VIEW_SUBMIT,
            &app,
            ActionPayload::DetailViewSubmit {
                normalized_url: "https://detail-next.com".to_string(),
                focused_node: Some(key),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert_eq!(intents.len(), 2);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://detail-next.com"
        ));
        assert!(matches!(
            intents.get(1),
            Some(GraphIntent::PromoteNodeToActive { key: selected, cause })
                if *selected == key && *cause == LifecycleCause::Restore
        ));
    }

    #[test]
    fn action_registry_pair_local_peer_emits_trust_peer_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng())
            .public()
            .to_string();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_VERSE_PAIR_DEVICE,
            &app,
            ActionPayload::VersePairDevice {
                mode: PairingMode::LocalPeer {
                    node_id: peer_id.clone(),
                },
            },
        );

        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::TrustPeer { peer_id: emitted, .. }) if emitted == &peer_id
        ));
    }

    #[test]
    fn action_registry_share_workspace_emits_grant_access_intents_for_trusted_peers() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_VERSE_SHARE_WORKSPACE,
            &app,
            ActionPayload::VerseShareWorkspace {
                workspace_id: "workspace:test".to_string(),
            },
        );

        match execution {
            ActionOutcome::Intents(intents) => {
                assert!(!intents.is_empty());
                assert!(intents.iter().all(|intent| {
                    matches!(
                        intent,
                        GraphIntent::GrantWorkspaceAccess { workspace_id, .. }
                            if workspace_id == "workspace:test"
                    )
                }));
            }
            ActionOutcome::Failure(ActionFailure { kind, reason }) => {
                assert_eq!(kind, ActionFailureKind::Rejected);
                assert!(reason.contains("no trusted peers"));
            }
        }
    }
}
