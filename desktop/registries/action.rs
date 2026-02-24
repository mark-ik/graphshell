use std::collections::HashMap;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause};
use crate::desktop::lifecycle_intents;
use crate::graph::NodeKey;
use crate::search::fuzzy_match_node_keys;
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
    OmniboxNodeSearch { query: String },
    GraphViewSubmit { input: String },
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

type ActionHandler = fn(&GraphBrowserApp, &ActionPayload) -> Vec<GraphIntent>;

#[derive(Debug)]
pub(crate) struct ActionExecution {
    pub(crate) action_id: String,
    pub(crate) intents: Vec<GraphIntent>,
    pub(crate) succeeded: bool,
}

pub(crate) struct ActionRegistry {
    handlers: HashMap<String, ActionHandler>,
}

impl ActionRegistry {
    pub(crate) fn register(&mut self, action_id: &str, handler: ActionHandler) {
        self.handlers.insert(action_id.to_ascii_lowercase(), handler);
    }

    pub(crate) fn execute(
        &self,
        action_id: &str,
        app: &GraphBrowserApp,
        payload: ActionPayload,
    ) -> ActionExecution {
        let normalized_action_id = action_id.to_ascii_lowercase();
        if let Some(handler) = self.handlers.get(&normalized_action_id) {
            return ActionExecution {
                action_id: normalized_action_id,
                intents: handler(app, &payload),
                succeeded: true,
            };
        }

        ActionExecution {
            action_id: normalized_action_id,
            intents: Vec::new(),
            succeeded: false,
        }
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        registry.register(ACTION_DETAIL_VIEW_SUBMIT, execute_detail_view_submit_action);
        registry.register(ACTION_GRAPH_VIEW_SUBMIT, execute_graph_view_submit_action);
        registry.register(ACTION_OMNIBOX_NODE_SEARCH, execute_omnibox_node_search_action);
        
        // Verse sync actions (Step 5.3)
        registry.register(ACTION_VERSE_PAIR_DEVICE, execute_verse_pair_device_action);
        registry.register(ACTION_VERSE_SYNC_NOW, execute_verse_sync_now_action);
        registry.register(ACTION_VERSE_SHARE_WORKSPACE, execute_verse_share_workspace_action);
        registry.register(ACTION_VERSE_FORGET_DEVICE, execute_verse_forget_device_action);
        
        registry
    }
}

fn execute_graph_view_submit_action(app: &GraphBrowserApp, payload: &ActionPayload) -> Vec<GraphIntent> {
    let ActionPayload::GraphViewSubmit { input } = payload else {
        return Vec::new();
    };

    let input = input.trim();
    if input.is_empty() {
        return Vec::new();
    }

    if let Some(selected_node) = app.get_single_selected_node() {
        vec![GraphIntent::SetNodeUrl {
            key: selected_node,
            new_url: input.to_string(),
        }]
    } else {
        let position = new_node_position_for_context(app, app.selected_nodes.primary());
        vec![GraphIntent::CreateNodeAtUrl {
            url: input.to_string(),
            position,
        }]
    }
}

fn execute_detail_view_submit_action(app: &GraphBrowserApp, payload: &ActionPayload) -> Vec<GraphIntent> {
    let ActionPayload::DetailViewSubmit {
        normalized_url,
        focused_node,
    } = payload
    else {
        return Vec::new();
    };

    if let Some(node_key) = focused_node {
        return vec![
            GraphIntent::SetNodeUrl {
                key: *node_key,
                new_url: normalized_url.clone(),
            },
            lifecycle_intents::promote_node_to_active(*node_key, LifecycleCause::Restore),
        ];
    }

    vec![GraphIntent::CreateNodeAtUrl {
        url: normalized_url.clone(),
        position: new_node_position_for_context(app, app.selected_nodes.primary()),
    }]
}

fn graph_centroid_or_default(app: &GraphBrowserApp) -> Point2D<f32> {
    if app.graph.node_count() == 0 {
        return Point2D::new(400.0, 300.0);
    }
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut count = 0.0f32;
    for (_, node) in app.graph.nodes() {
        sum_x += node.position.x;
        sum_y += node.position.y;
        count += 1.0;
    }
    Point2D::new(sum_x / count, sum_y / count)
}

fn new_node_position_for_context(app: &GraphBrowserApp, anchor: Option<NodeKey>) -> Point2D<f32> {
    let base = anchor
        .and_then(|key| app.graph.get_node(key).map(|node| node.position))
        .unwrap_or_else(|| graph_centroid_or_default(app));
    let n = app.graph.node_count() as f32;
    let angle = n * 0.7853982;
    let radius = 90.0;
    Point2D::new(base.x + radius * angle.cos(), base.y + radius * angle.sin())
}

fn execute_omnibox_node_search_action(app: &GraphBrowserApp, payload: &ActionPayload) -> Vec<GraphIntent> {
    let ActionPayload::OmniboxNodeSearch { query } = payload else {
        return Vec::new();
    };

    let matched_keys = fuzzy_match_node_keys(&app.graph, query);
    if let Some(key) = matched_keys.first() {
        return vec![GraphIntent::SelectNode {
            key: *key,
            multi_select: false,
        }];
    }
    Vec::new()
}

// ===== Verse Sync Action Handlers (Step 5.3) =====

fn execute_verse_pair_device_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> Vec<GraphIntent> {
    let ActionPayload::VersePairDevice { mode } = payload else {
        return Vec::new();
    };
    
    // For Step 5.3: Generate pairing code or initiate connection
    // The actual UI dialog is handled by the GUI layer (Step 5.3 UI implementation)
    // This action just triggers the pairing state machine
    match mode {
        PairingMode::ShowCode => {
            // Generate pairing code - the GUI will read this via verse::generate_pairing_code()
            log::info!("Pairing code requested - UI will call verse::generate_pairing_code()");
            Vec::new() // No intents emitted - this is a UI state change
        }
        PairingMode::EnterCode { code } => {
            // Decode and attempt connection
            log::info!("Pairing initiated with code: {}", code);
            // Step 5.4 will implement the actual connection logic
            Vec::new()
        }
        PairingMode::LocalPeer { node_id } => {
            // Connect to discovered mDNS peer
            log::info!("Pairing initiated with local peer: {}", node_id);
            // Step 5.4 will implement the actual connection logic
            Vec::new()
        }
    }
}

fn execute_verse_sync_now_action(_app: &GraphBrowserApp, _payload: &ActionPayload) -> Vec<GraphIntent> {
    // Step 5.4 will implement the sync worker trigger
    log::info!("Manual sync requested");
    Vec::new()
}

fn execute_verse_share_workspace_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> Vec<GraphIntent> {
    let ActionPayload::VerseShareWorkspace { workspace_id } = payload else {
        return Vec::new();
    };
    
    // Step 5.5 will implement workspace grant management
    log::info!("Share workspace requested for: {}", workspace_id);
    Vec::new()
}

fn execute_verse_forget_device_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> Vec<GraphIntent> {
    let ActionPayload::VerseForgetDevice { node_id } = payload else {
        return Vec::new();
    };
    
    // Revoke trust for this peer
    if let Ok(node_id_parsed) = node_id.parse::<iroh::NodeId>() {
        crate::mods::native::verse::revoke_peer(node_id_parsed);
        log::info!("Device forgotten: {}", node_id);
    }
    Vec::new()
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
            .graph
            .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.graph.get_node_mut(key) {
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

        assert!(execution.succeeded);
        assert_eq!(execution.action_id, ACTION_OMNIBOX_NODE_SEARCH);
        assert_eq!(execution.intents.len(), 1);
        assert!(matches!(
            execution.intents.first(),
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

        assert!(!execution.succeeded);
        assert!(execution.intents.is_empty());
        assert_eq!(execution.action_id, "action.unknown");
    }

    #[test]
    fn action_registry_executes_graph_view_submit_action() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .graph
            .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));
        app.selected_nodes.select(key, false);

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_GRAPH_VIEW_SUBMIT,
            &app,
            ActionPayload::GraphViewSubmit {
                input: "https://next.com".to_string(),
            },
        );

        assert!(execution.succeeded);
        assert_eq!(execution.action_id, ACTION_GRAPH_VIEW_SUBMIT);
        assert!(matches!(
            execution.intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://next.com"
        ));
    }

    #[test]
    fn action_registry_executes_detail_view_submit_action_for_focused_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
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

        assert!(execution.succeeded);
        assert_eq!(execution.action_id, ACTION_DETAIL_VIEW_SUBMIT);
        assert_eq!(execution.intents.len(), 2);
        assert!(matches!(
            execution.intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://detail-next.com"
        ));
        assert!(matches!(
            execution.intents.get(1),
            Some(GraphIntent::PromoteNodeToActive { key: selected, cause })
                if *selected == key && *cause == LifecycleCause::Restore
        ));
    }
}
