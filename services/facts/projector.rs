/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Projection boundary: `LogEntry` → `ProjectedFact`.
//!
//! Spec: `2026-03-18_fact_query_type_sketch.md §5`
//!
//! Design rules:
//! - Projection is pure and deterministic: same log position + entry ⇒ same facts.
//! - `FactId` is derived from `(log_position, ordinal)` for stable rebuild identity.
//! - Unknown / non-history `LogEntry` variants project to an empty list (not an error).

use crate::services::facts::types::{
    EventSourceRef, FactEnvelope, FactId, ProjectedFact, ProjectedFactKind,
};
use crate::services::persistence::types::LogEntry;

/// Pure projection boundary from a single WAL entry to zero or more facts.
pub trait FactProjector {
    fn project(&self, log_position: u64, entry: &LogEntry) -> Vec<ProjectedFact>;
}

/// First-slice projector covering the current history event families:
/// `AddNode`, `RemoveNode`, `NavigateNode`, `AppendNodeAuditEvent`, `AppendTraversal`.
pub struct HistoryFactProjector;

impl FactProjector for HistoryFactProjector {
    fn project(&self, log_position: u64, entry: &LogEntry) -> Vec<ProjectedFact> {
        match entry {
            LogEntry::AppendTraversal {
                from_node_id,
                to_node_id,
                trigger,
                timestamp_ms,
            } => vec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId::from_position(log_position, 0),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::Traversal {
                    from_node_id: from_node_id.clone(),
                    to_node_id: to_node_id.clone(),
                    trigger: *trigger,
                },
            }],

            LogEntry::NavigateNode {
                node_id,
                from_url,
                to_url,
                trigger,
                timestamp_ms,
            } => vec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId::from_position(log_position, 0),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::NodeNavigation {
                    node_id: node_id.clone(),
                    from_url: from_url.clone(),
                    to_url: to_url.clone(),
                    trigger: *trigger,
                },
            }],

            LogEntry::AppendNodeAuditEvent {
                node_id,
                event,
                timestamp_ms,
            } => vec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId::from_position(log_position, 0),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::NodeAudit {
                    node_id: node_id.clone(),
                    event: event.clone(),
                },
            }],

            LogEntry::AddNode {
                node_id,
                timestamp_ms,
                ..
            } => vec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId::from_position(log_position, 0),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::GraphStructure {
                    node_id: node_id.clone(),
                    is_addition: true,
                },
            }],

            LogEntry::RemoveNode {
                node_id,
                timestamp_ms,
            } => vec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId::from_position(log_position, 0),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::GraphStructure {
                    node_id: node_id.clone(),
                    is_addition: false,
                },
            }],

            // All other LogEntry variants are non-history mutations (structural
            // graph edits, tag updates, URL updates, etc.) — no history fact
            // projected in the first slice.
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::persistence::types::{NodeAuditEventKind, PersistedNavigationTrigger};

    fn projector() -> HistoryFactProjector {
        HistoryFactProjector
    }

    #[test]
    fn projects_append_traversal() {
        let entry = LogEntry::AppendTraversal {
            from_node_id: "a".to_string(),
            to_node_id: "b".to_string(),
            timestamp_ms: 1000,
            trigger: PersistedNavigationTrigger::LinkClick,
        };
        let facts = projector().project(7, &entry);
        assert_eq!(facts.len(), 1);
        assert!(
            matches!(&facts[0].kind, ProjectedFactKind::Traversal { from_node_id, to_node_id, .. }
                if from_node_id == "a" && to_node_id == "b")
        );
        assert_eq!(facts[0].envelope.timestamp_ms, 1000);
        assert_eq!(facts[0].envelope.source.log_position, 7);
    }

    #[test]
    fn projects_navigate_node() {
        let entry = LogEntry::NavigateNode {
            node_id: "n1".to_string(),
            from_url: "https://a.test/".to_string(),
            to_url: "https://b.test/".to_string(),
            trigger: PersistedNavigationTrigger::Back,
            timestamp_ms: 2000,
        };
        let facts = projector().project(3, &entry);
        assert_eq!(facts.len(), 1);
        assert!(
            matches!(&facts[0].kind, ProjectedFactKind::NodeNavigation { node_id, .. }
                if node_id == "n1")
        );
    }

    #[test]
    fn projects_node_audit_event() {
        let entry = LogEntry::AppendNodeAuditEvent {
            node_id: "n2".to_string(),
            event: NodeAuditEventKind::Pinned,
            timestamp_ms: 3000,
        };
        let facts = projector().project(5, &entry);
        assert_eq!(facts.len(), 1);
        assert!(
            matches!(&facts[0].kind, ProjectedFactKind::NodeAudit { node_id, .. }
            if node_id == "n2")
        );
    }

    #[test]
    fn projects_add_node_as_graph_structure_addition() {
        let entry = LogEntry::AddNode {
            node_id: "n3".to_string(),
            url: "https://c.test/".to_string(),
            position_x: 0.0,
            position_y: 0.0,
            timestamp_ms: 500,
        };
        let facts = projector().project(1, &entry);
        assert_eq!(facts.len(), 1);
        assert!(matches!(
            &facts[0].kind,
            ProjectedFactKind::GraphStructure {
                is_addition: true,
                ..
            }
        ));
    }

    #[test]
    fn projects_remove_node_as_graph_structure_removal() {
        let entry = LogEntry::RemoveNode {
            node_id: "n4".to_string(),
            timestamp_ms: 600,
        };
        let facts = projector().project(2, &entry);
        assert_eq!(facts.len(), 1);
        assert!(matches!(
            &facts[0].kind,
            ProjectedFactKind::GraphStructure {
                is_addition: false,
                ..
            }
        ));
    }

    #[test]
    fn non_history_entry_projects_empty() {
        let entry = LogEntry::UpdateNodeTitle {
            node_id: "n5".to_string(),
            title: "New Title".to_string(),
        };
        let facts = projector().project(10, &entry);
        assert!(facts.is_empty());
    }

    #[test]
    fn fact_ids_are_deterministic_across_projections() {
        let entry = LogEntry::AppendTraversal {
            from_node_id: "x".to_string(),
            to_node_id: "y".to_string(),
            timestamp_ms: 42,
            trigger: PersistedNavigationTrigger::Unknown,
        };
        let facts_a = projector().project(99, &entry);
        let facts_b = projector().project(99, &entry);
        assert_eq!(facts_a[0].envelope.fact_id, facts_b[0].envelope.fact_id);
    }

    #[test]
    fn fact_ids_differ_for_different_log_positions() {
        let entry = LogEntry::AppendTraversal {
            from_node_id: "x".to_string(),
            to_node_id: "y".to_string(),
            timestamp_ms: 42,
            trigger: PersistedNavigationTrigger::Unknown,
        };
        let facts_a = projector().project(1, &entry);
        let facts_b = projector().project(2, &entry);
        assert_ne!(facts_a[0].envelope.fact_id, facts_b[0].envelope.fact_id);
    }
}

