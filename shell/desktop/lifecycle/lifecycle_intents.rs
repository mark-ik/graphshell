/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::app::{LifecycleCause, RuntimeEvent};
use crate::graph::NodeKey;

pub(crate) fn promote_node_to_active(key: NodeKey, cause: LifecycleCause) -> RuntimeEvent {
    RuntimeEvent::PromoteNodeToActive { key, cause }
}

pub(crate) fn demote_node_to_warm(key: NodeKey, cause: LifecycleCause) -> RuntimeEvent {
    RuntimeEvent::DemoteNodeToWarm { key, cause }
}

pub(crate) fn demote_node_to_cold(key: NodeKey, cause: LifecycleCause) -> RuntimeEvent {
    RuntimeEvent::DemoteNodeToCold { key, cause }
}

#[cfg(test)]
mod tests {
    use super::{LifecycleCause, demote_node_to_cold, demote_node_to_warm, promote_node_to_active};
    use crate::app::RuntimeEvent;
    use crate::graph::NodeKey;

    #[test]
    fn test_lifecycle_intent_adapter_maps_promote() {
        let key = NodeKey::new(1);
        let intent = promote_node_to_active(key, LifecycleCause::UserSelect);
        assert!(
            matches!(intent, RuntimeEvent::PromoteNodeToActive { key: k, cause } if k == key && cause == LifecycleCause::UserSelect)
        );
    }

    #[test]
    fn test_lifecycle_intent_adapter_maps_demote_warm() {
        let key = NodeKey::new(2);
        let intent = demote_node_to_warm(key, LifecycleCause::WorkspaceRetention);
        assert!(
            matches!(intent, RuntimeEvent::DemoteNodeToWarm { key: k, cause } if k == key && cause == LifecycleCause::WorkspaceRetention)
        );
    }

    #[test]
    fn test_lifecycle_intent_adapter_maps_demote_cold() {
        let key = NodeKey::new(3);
        let intent = demote_node_to_cold(key, LifecycleCause::ActiveLruEviction);
        assert!(
            matches!(intent, RuntimeEvent::DemoteNodeToCold { key: k, cause } if k == key && cause == LifecycleCause::ActiveLruEviction)
        );
    }
}

