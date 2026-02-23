use std::collections::HashMap;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause};
use crate::desktop::lifecycle_intents;
use crate::graph::NodeKey;
use crate::search::fuzzy_match_node_keys;
use euclid::default::Point2D;

pub(crate) const ACTION_OMNIBOX_NODE_SEARCH: &str = "action.omnibox_node_search";
pub(crate) const ACTION_GRAPH_VIEW_SUBMIT: &str = "action.graph_view_submit";
pub(crate) const ACTION_DETAIL_VIEW_SUBMIT: &str = "action.detail_view_submit";

#[derive(Debug, Clone)]
pub(crate) enum ActionPayload {
    OmniboxNodeSearch { query: String },
    GraphViewSubmit { input: String },
    DetailViewSubmit {
        normalized_url: String,
        focused_node: Option<NodeKey>,
    },
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

    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    fuzzy_match_node_keys(&app.graph, query)
        .first()
        .copied()
        .map(|key| {
            vec![GraphIntent::SelectNode {
                key,
                multi_select: false,
            }]
        })
        .unwrap_or_default()
}

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
