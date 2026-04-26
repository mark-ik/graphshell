/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph Cartography runtime invalidation adapters.
//!
//! This module keeps the root app's `GraphIntent` vocabulary outside the
//! `graph-cartography` crate while still letting reducer events emit typed GC
//! invalidation records.

use ::graph_cartography::CartographyRuntimeInvalidationEvent;

use super::*;

impl GraphBrowserApp {
    pub(crate) fn record_cartography_invalidation_for_intent(&mut self, intent: &GraphIntent) {
        let Some(event) = cartography_event_for_intent(intent) else {
            return;
        };
        self.cartography_invalidation_emitter
            .emit_runtime_event(event);
    }
}

fn cartography_event_for_intent(
    intent: &GraphIntent,
) -> Option<CartographyRuntimeInvalidationEvent> {
    match intent {
        GraphIntent::CreateNodeNearCenter
        | GraphIntent::CreateNodeNearCenterAndOpen { .. }
        | GraphIntent::CreateNodeAtUrl { .. }
        | GraphIntent::CreateNodeAtUrlAndOpen { .. }
        | GraphIntent::AcceptHostOpenRequest { .. } => {
            Some(CartographyRuntimeInvalidationEvent::GraphReset)
        }
        GraphIntent::CreateNoteForNode { key, .. } => {
            Some(CartographyRuntimeInvalidationEvent::GraphNodeAdded {
                node: *key,
                entry: None,
            })
        }
        GraphIntent::RemoveSelectedNodes | GraphIntent::MarkTombstoneForSelected => {
            Some(CartographyRuntimeInvalidationEvent::GraphReset)
        }
        GraphIntent::RestoreGhostNode { .. } => {
            Some(CartographyRuntimeInvalidationEvent::LifecycleCold { entry: None })
        }
        GraphIntent::ClearGraph => Some(CartographyRuntimeInvalidationEvent::GraphReset),
        GraphIntent::CreateUserGroupedEdge { from, .. } => {
            Some(CartographyRuntimeInvalidationEvent::GraphEdgeAsserted {
                node: Some(*from),
                entry: None,
            })
        }
        GraphIntent::CreateUserGroupedEdgeFromPrimarySelection => {
            Some(CartographyRuntimeInvalidationEvent::GraphEdgeAsserted {
                node: None,
                entry: None,
            })
        }
        GraphIntent::RemoveEdge { from, .. } => {
            Some(CartographyRuntimeInvalidationEvent::GraphEdgeAsserted {
                node: Some(*from),
                entry: None,
            })
        }
        GraphIntent::SetNodeUrl { key, .. }
        | GraphIntent::UpdateNodeMimeHint { key, .. }
        | GraphIntent::UpdateNodeViewerOverride { key, .. } => {
            Some(CartographyRuntimeInvalidationEvent::GraphNodeAdded {
                node: *key,
                entry: None,
            })
        }
        GraphIntent::TagNode { key, .. }
        | GraphIntent::UntagNode { key, .. }
        | GraphIntent::AssignClassification { key, .. }
        | GraphIntent::UnassignClassification { key, .. }
        | GraphIntent::AcceptClassification { key, .. }
        | GraphIntent::RejectClassification { key, .. }
        | GraphIntent::SetPrimaryClassification { key, .. } => {
            Some(CartographyRuntimeInvalidationEvent::GraphTagChanged {
                node: *key,
                entry: None,
            })
        }
        GraphIntent::PromoteNodeToActive { .. } => {
            Some(CartographyRuntimeInvalidationEvent::LifecycleActive { entry: None })
        }
        GraphIntent::DemoteNodeToWarm { .. } => {
            Some(CartographyRuntimeInvalidationEvent::LifecycleWarm { entry: None })
        }
        GraphIntent::DemoteNodeToCold { .. } => {
            Some(CartographyRuntimeInvalidationEvent::LifecycleCold { entry: None })
        }
        GraphIntent::WebViewUrlChanged { .. } | GraphIntent::WebViewHistoryChanged { .. } => {
            Some(CartographyRuntimeInvalidationEvent::WalNavigateNode { entry: None })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::graph_cartography::{DeterministicAggregateKind, GraphTruthMutationKind};

    #[test]
    fn cartography_adapter_maps_existing_runtime_lifecycle_intents() {
        let node = crate::graph::NodeKey::new(7);
        let event = cartography_event_for_intent(&GraphIntent::PromoteNodeToActive {
            key: node,
            cause: LifecycleCause::UserSelect,
        })
        .expect("active lifecycle intent should emit GC invalidation");

        assert!(matches!(
            event,
            CartographyRuntimeInvalidationEvent::LifecycleActive { entry: None }
        ));
    }

    #[test]
    fn cartography_adapter_maps_existing_graph_mutation_intents() {
        let node = crate::graph::NodeKey::new(7);
        let event = cartography_event_for_intent(&GraphIntent::CreateUserGroupedEdge {
            from: node,
            to: crate::graph::NodeKey::new(8),
            label: None,
        })
        .expect("edge mutation should emit GC invalidation");

        assert!(matches!(
            event,
            CartographyRuntimeInvalidationEvent::GraphEdgeAsserted {
                node: Some(mapped),
                entry: None,
            } if mapped == node
        ));
    }

    #[test]
    fn graph_app_queues_cartography_invalidations_from_reducer() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.apply_reducer_intents([GraphIntent::TagNode {
            key: crate::graph::NodeKey::new(1),
            tag: "test".into(),
        }]);

        let pending = app.pending_cartography_invalidations();
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending[0].signal,
            ::graph_cartography::CartographyInvalidationSignal::GraphTruthMutation {
                kind: GraphTruthMutationKind::TagChange,
                ..
            }
        ));
        assert!(
            pending[0]
                .plan
                .deterministic
                .contains(&DeterministicAggregateKind::ActivationFreshness)
        );

        assert_eq!(app.drain_cartography_invalidations().len(), 1);
        assert!(app.pending_cartography_invalidations().is_empty());
    }
}
