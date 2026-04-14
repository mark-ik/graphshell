/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Query engine: answers structured [`GraphQuery`]s over the [`FactStore`].
//!
//! Spec: `2026-03-18_fact_query_type_sketch.md §7.3`
//!
//! Design rules:
//! - `execute()` filters facts, sorts newest-first (`timestamp_ms DESC, log_position DESC`),
//!   and adapts results back to existing surface types.
//! - No WAL access — reads only from `FactStore`.
//! - No mutation.

pub mod adapters;
pub mod types;

use crate::services::facts::FactStore;
use crate::services::facts::types::{ProjectedFact, ProjectedFactKind};
use crate::services::query::adapters::{
    fact_matches_text, fact_to_audit_entry, fact_to_navigate_node_entry, fact_to_timeline_event,
};
use crate::services::query::types::{FactQueryFilter, GraphQuery, GraphQueryResult};

/// Pure query engine over a [`FactStore`].
///
/// Owned by [`GraphStore`]; constructed after WAL rebuild.
///
/// [`GraphStore`]: crate::services::persistence::GraphStore
pub struct GraphQueryEngine {
    facts: FactStore,
}

impl GraphQueryEngine {
    pub fn new(facts: FactStore) -> Self {
        Self { facts }
    }

    /// Access the underlying [`FactStore`] (read-only).
    pub fn fact_store(&self) -> &FactStore {
        &self.facts
    }

    /// Access the underlying [`FactStore`] mutably (for incremental append).
    pub(crate) fn fact_store_mut(&mut self) -> &mut FactStore {
        &mut self.facts
    }

    /// Execute a structured query and return adapted results.
    pub fn execute(&self, query: GraphQuery) -> GraphQueryResult {
        match query {
            GraphQuery::MixedTimeline { filter, limit } => {
                GraphQueryResult::TimelineEvents(self.mixed_timeline(&filter, limit))
            }
            GraphQuery::NodeNavigationHistory { node_id, limit } => {
                GraphQueryResult::NodeNavigationEntries(
                    self.node_navigation_history(&node_id, limit),
                )
            }
            GraphQuery::NodeAuditHistory { node_id, limit } => {
                GraphQueryResult::NodeAuditEntries(self.node_audit_history(&node_id, limit))
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Internal query implementations
    // ──────────────────────────────────────────────────────────────────────

    fn mixed_timeline(
        &self,
        filter: &FactQueryFilter,
        limit: usize,
    ) -> Vec<crate::services::persistence::types::HistoryTimelineEvent> {
        if limit == 0 {
            return Vec::new();
        }

        let mut rows: Vec<&ProjectedFact> = self
            .facts
            .facts()
            .iter()
            .filter(|f| self.matches_filter(f, filter))
            .collect();

        // Sort newest-first
        rows.sort_by(|a, b| {
            b.envelope
                .timestamp_ms
                .cmp(&a.envelope.timestamp_ms)
                .then_with(|| {
                    b.envelope
                        .source
                        .log_position
                        .cmp(&a.envelope.source.log_position)
                })
        });

        rows.truncate(limit);

        rows.iter()
            .filter_map(|f| fact_to_timeline_event(f))
            .collect()
    }

    fn node_navigation_history(
        &self,
        node_id: &str,
        limit: usize,
    ) -> Vec<crate::services::persistence::types::LogEntry> {
        if limit == 0 {
            return Vec::new();
        }

        let mut rows: Vec<&ProjectedFact> = self
            .facts
            .positions_for_node(node_id)
            .iter()
            .filter_map(|&idx| self.facts.facts().get(idx))
            .filter(|f| matches!(f.kind, ProjectedFactKind::NodeNavigation { .. }))
            .collect();

        rows.sort_by(|a, b| {
            b.envelope
                .timestamp_ms
                .cmp(&a.envelope.timestamp_ms)
                .then_with(|| {
                    b.envelope
                        .source
                        .log_position
                        .cmp(&a.envelope.source.log_position)
                })
        });

        rows.truncate(limit);
        rows.iter()
            .filter_map(|f| fact_to_navigate_node_entry(f))
            .collect()
    }

    fn node_audit_history(
        &self,
        node_id: &str,
        limit: usize,
    ) -> Vec<crate::services::persistence::types::LogEntry> {
        if limit == 0 {
            return Vec::new();
        }

        let mut rows: Vec<&ProjectedFact> = self
            .facts
            .positions_for_node(node_id)
            .iter()
            .filter_map(|&idx| self.facts.facts().get(idx))
            .filter(|f| matches!(f.kind, ProjectedFactKind::NodeAudit { .. }))
            .collect();

        rows.sort_by(|a, b| {
            b.envelope
                .timestamp_ms
                .cmp(&a.envelope.timestamp_ms)
                .then_with(|| {
                    b.envelope
                        .source
                        .log_position
                        .cmp(&a.envelope.source.log_position)
                })
        });

        rows.truncate(limit);
        rows.iter().filter_map(|f| fact_to_audit_entry(f)).collect()
    }

    // ──────────────────────────────────────────────────────────────────────
    // Filter predicate
    // ──────────────────────────────────────────────────────────────────────

    fn matches_filter(&self, fact: &ProjectedFact, filter: &FactQueryFilter) -> bool {
        // Kind filter
        if let Some(ref kinds) = filter.kinds {
            if !kinds.is_empty() && !kinds.contains(&fact.kind.discriminant()) {
                return false;
            }
        }

        // node_id filter — for Traversal, check both from and to
        if let Some(ref filter_node_id) = filter.node_id {
            let node_matches = match &fact.kind {
                ProjectedFactKind::Traversal {
                    from_node_id,
                    to_node_id,
                    ..
                } => from_node_id == filter_node_id || to_node_id == filter_node_id,
                ProjectedFactKind::NodeNavigation { node_id, .. } => node_id == filter_node_id,
                ProjectedFactKind::NodeAudit { node_id, .. } => node_id == filter_node_id,
                ProjectedFactKind::GraphStructure { node_id, .. } => node_id == filter_node_id,
            };
            if !node_matches {
                return false;
            }
        }

        // Temporal range filters
        if let Some(after_ms) = filter.after_ms {
            if fact.envelope.timestamp_ms < after_ms {
                return false;
            }
        }
        if let Some(before_ms) = filter.before_ms {
            if fact.envelope.timestamp_ms > before_ms {
                return false;
            }
        }

        // Text filter (most expensive — runs last)
        if let Some(ref needle) = filter.text_contains {
            if !fact_matches_text(fact, &needle.to_lowercase()) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::facts::projector::HistoryFactProjector;
    use crate::services::facts::types::ProjectedFactDiscriminant;
    use crate::services::persistence::types::{
        LogEntry, NodeAuditEventKind, PersistedNavigationTrigger,
    };

    fn engine_from_entries(entries: Vec<(u64, LogEntry)>) -> GraphQueryEngine {
        let projector = HistoryFactProjector;
        let store = FactStore::rebuild_from_log(&projector, entries.into_iter());
        GraphQueryEngine::new(store)
    }

    fn traversal(pos: u64, from: &str, to: &str, ts: u64) -> (u64, LogEntry) {
        (
            pos,
            LogEntry::AppendTraversal {
                from_node_id: from.to_string(),
                to_node_id: to.to_string(),
                timestamp_ms: ts,
                trigger: PersistedNavigationTrigger::LinkClick,
            },
        )
    }

    #[test]
    fn mixed_timeline_returns_newest_first() {
        let engine = engine_from_entries(vec![
            traversal(1, "a", "b", 100),
            traversal(2, "b", "c", 300),
            traversal(3, "c", "d", 200),
        ]);
        let result = engine
            .execute(GraphQuery::MixedTimeline {
                filter: FactQueryFilter::default(),
                limit: 10,
            })
            .into_timeline_events();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].timestamp_ms, 300);
        assert_eq!(result[1].timestamp_ms, 200);
        assert_eq!(result[2].timestamp_ms, 100);
    }

    #[test]
    fn mixed_timeline_respects_limit() {
        let engine = engine_from_entries(vec![
            traversal(1, "a", "b", 100),
            traversal(2, "b", "c", 200),
            traversal(3, "c", "d", 300),
        ]);
        let result = engine
            .execute(GraphQuery::MixedTimeline {
                filter: FactQueryFilter::default(),
                limit: 2,
            })
            .into_timeline_events();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn mixed_timeline_zero_limit_returns_empty() {
        let engine = engine_from_entries(vec![traversal(1, "a", "b", 100)]);
        let result = engine
            .execute(GraphQuery::MixedTimeline {
                filter: FactQueryFilter::default(),
                limit: 0,
            })
            .into_timeline_events();
        assert!(result.is_empty());
    }

    #[test]
    fn mixed_timeline_kind_filter_excludes_non_matching() {
        let engine = engine_from_entries(vec![
            traversal(1, "a", "b", 100),
            (
                2,
                LogEntry::AddNode {
                    node_id: "n1".to_string(),
                    url: "https://a.test/".to_string(),
                    position_x: 0.0,
                    position_y: 0.0,
                    timestamp_ms: 200,
                },
            ),
        ]);
        let result = engine
            .execute(GraphQuery::MixedTimeline {
                filter: FactQueryFilter {
                    kinds: Some(vec![ProjectedFactDiscriminant::GraphStructure]),
                    ..Default::default()
                },
                limit: 10,
            })
            .into_timeline_events();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn node_navigation_history_filters_by_node() {
        let engine = engine_from_entries(vec![
            (
                1,
                LogEntry::NavigateNode {
                    node_id: "n1".to_string(),
                    from_url: "https://a.test/".to_string(),
                    to_url: "https://b.test/".to_string(),
                    trigger: PersistedNavigationTrigger::Back,
                    timestamp_ms: 100,
                },
            ),
            (
                2,
                LogEntry::NavigateNode {
                    node_id: "n2".to_string(),
                    from_url: "https://c.test/".to_string(),
                    to_url: "https://d.test/".to_string(),
                    trigger: PersistedNavigationTrigger::Back,
                    timestamp_ms: 200,
                },
            ),
        ]);
        let result = engine
            .execute(GraphQuery::NodeNavigationHistory {
                node_id: "n1".to_string(),
                limit: 10,
            })
            .into_node_navigation_entries();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], LogEntry::NavigateNode { node_id, .. } if node_id == "n1"));
    }

    #[test]
    fn node_audit_history_filters_by_node() {
        let engine = engine_from_entries(vec![
            (
                1,
                LogEntry::AppendNodeAuditEvent {
                    node_id: "n1".to_string(),
                    event: NodeAuditEventKind::Pinned,
                    timestamp_ms: 100,
                },
            ),
            (
                2,
                LogEntry::AppendNodeAuditEvent {
                    node_id: "n2".to_string(),
                    event: NodeAuditEventKind::Unpinned,
                    timestamp_ms: 200,
                },
            ),
        ]);
        let result = engine
            .execute(GraphQuery::NodeAuditHistory {
                node_id: "n1".to_string(),
                limit: 10,
            })
            .into_node_audit_entries();
        assert_eq!(result.len(), 1);
        assert!(
            matches!(&result[0], LogEntry::AppendNodeAuditEvent { node_id, .. } if node_id == "n1")
        );
    }

    #[test]
    fn mixed_timeline_node_id_filter_matches_traversal_endpoints() {
        let engine = engine_from_entries(vec![
            traversal(1, "target", "other", 100),
            traversal(2, "unrelated", "other2", 200),
        ]);
        let result = engine
            .execute(GraphQuery::MixedTimeline {
                filter: FactQueryFilter {
                    node_id: Some("target".to_string()),
                    ..Default::default()
                },
                limit: 10,
            })
            .into_timeline_events();
        assert_eq!(result.len(), 1);
    }
}

