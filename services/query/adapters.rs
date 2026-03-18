/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Adapter helpers: convert between existing persistence surface types and
//! first-slice fact/query types.
//!
//! Spec: `2026-03-18_fact_query_type_sketch.md §8`
//!
//! Migration rule: preserve public behavior first, move ownership second.
//! These adapters allow [`GraphStore`] methods to delegate to the query layer
//! without changing their public signatures.
//!
//! [`GraphStore`]: crate::services::persistence::GraphStore

use crate::services::facts::types::{ProjectedFact, ProjectedFactDiscriminant, ProjectedFactKind};
use crate::services::persistence::types::{
    HistoryEventKind, HistoryTimelineEvent, HistoryTimelineFilter, HistoryTrackKind, LogEntry,
    NodeAuditEventKind,
};
use crate::services::query::types::FactQueryFilter;

// ──────────────────────────────────────────────────────────────────────────
// Filter adapters
// ──────────────────────────────────────────────────────────────────────────

/// Convert a [`HistoryTimelineFilter`] to a [`FactQueryFilter`] for `MixedTimeline` queries.
pub fn fact_filter_from_history_filter(filter: &HistoryTimelineFilter) -> FactQueryFilter {
    FactQueryFilter {
        kinds: filter.tracks.as_ref().map(|tracks| {
            tracks
                .iter()
                .map(|t| match t {
                    HistoryTrackKind::Traversal => ProjectedFactDiscriminant::Traversal,
                    HistoryTrackKind::NodeNavigation => ProjectedFactDiscriminant::NodeNavigation,
                    HistoryTrackKind::NodeAudit => ProjectedFactDiscriminant::NodeAudit,
                    HistoryTrackKind::GraphStructure => ProjectedFactDiscriminant::GraphStructure,
                })
                .collect()
        }),
        node_id: filter.node_id.clone(),
        after_ms: filter.after_ms,
        before_ms: filter.before_ms,
        text_contains: filter.text_contains.clone(),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Result adapters: ProjectedFact → existing surface types
// ──────────────────────────────────────────────────────────────────────────

/// Convert a [`ProjectedFact`] to a [`HistoryTimelineEvent`] for `MixedTimeline` results.
///
/// Returns `None` for fact kinds that do not map to a timeline event
/// (should not happen for the first-slice kinds, but keeps the signature honest).
pub fn fact_to_timeline_event(fact: &ProjectedFact) -> Option<HistoryTimelineEvent> {
    let kind = match &fact.kind {
        ProjectedFactKind::Traversal {
            from_node_id,
            to_node_id,
            trigger,
        } => HistoryEventKind::Traversal {
            from_node_id: from_node_id.clone(),
            to_node_id: to_node_id.clone(),
            trigger: *trigger,
        },
        ProjectedFactKind::NodeNavigation {
            node_id,
            from_url,
            to_url,
            trigger,
        } => HistoryEventKind::NodeNavigation {
            node_id: node_id.clone(),
            from_url: from_url.clone(),
            to_url: to_url.clone(),
            trigger: *trigger,
        },
        ProjectedFactKind::NodeAudit { node_id, event } => HistoryEventKind::NodeAudit {
            node_id: node_id.clone(),
            event: event.clone(),
        },
        ProjectedFactKind::GraphStructure { node_id, is_addition } => {
            HistoryEventKind::GraphStructure {
                node_id: node_id.clone(),
                is_addition: *is_addition,
            }
        }
    };

    Some(HistoryTimelineEvent {
        timestamp_ms: fact.envelope.timestamp_ms,
        log_position: fact.envelope.source.log_position,
        kind,
    })
}

/// Convert a `NodeNavigation` [`ProjectedFact`] back to a [`LogEntry::NavigateNode`].
///
/// Returns `None` if the fact is not a `NodeNavigation` variant.
pub fn fact_to_navigate_node_entry(fact: &ProjectedFact) -> Option<LogEntry> {
    if let ProjectedFactKind::NodeNavigation {
        node_id,
        from_url,
        to_url,
        trigger,
    } = &fact.kind
    {
        Some(LogEntry::NavigateNode {
            node_id: node_id.clone(),
            from_url: from_url.clone(),
            to_url: to_url.clone(),
            trigger: *trigger,
            timestamp_ms: fact.envelope.timestamp_ms,
        })
    } else {
        None
    }
}

/// Convert a `NodeAudit` [`ProjectedFact`] back to a [`LogEntry::AppendNodeAuditEvent`].
///
/// Returns `None` if the fact is not a `NodeAudit` variant.
pub fn fact_to_audit_entry(fact: &ProjectedFact) -> Option<LogEntry> {
    if let ProjectedFactKind::NodeAudit { node_id, event } = &fact.kind {
        Some(LogEntry::AppendNodeAuditEvent {
            node_id: node_id.clone(),
            event: event.clone(),
            timestamp_ms: fact.envelope.timestamp_ms,
        })
    } else {
        None
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Text-match helper (mirrors existing mixed_timeline_entries logic)
// ──────────────────────────────────────────────────────────────────────────

/// Case-insensitive text match against the fact's display-text projection.
pub fn fact_matches_text(fact: &ProjectedFact, needle_lower: &str) -> bool {
    let haystack = match &fact.kind {
        ProjectedFactKind::Traversal {
            from_node_id,
            to_node_id,
            ..
        } => format!("{} {}", from_node_id, to_node_id),
        ProjectedFactKind::NodeNavigation {
            from_url, to_url, ..
        } => format!("{} {}", from_url, to_url),
        ProjectedFactKind::NodeAudit { event, .. } => match event {
            NodeAuditEventKind::TitleChanged { new_title } => {
                format!("TitleChanged {}", new_title)
            }
            NodeAuditEventKind::Tagged { tag } => format!("Tagged {}", tag),
            NodeAuditEventKind::Untagged { tag } => format!("Untagged {}", tag),
            NodeAuditEventKind::UrlChanged { new_url } => format!("UrlChanged {}", new_url),
            NodeAuditEventKind::Pinned => "Pinned".to_string(),
            NodeAuditEventKind::Unpinned => "Unpinned".to_string(),
            NodeAuditEventKind::Tombstoned => "Tombstoned".to_string(),
            NodeAuditEventKind::Restored => "Restored".to_string(),
        },
        ProjectedFactKind::GraphStructure {
            node_id,
            is_addition,
        } => {
            format!(
                "{} {}",
                node_id,
                if *is_addition { "added" } else { "removed" }
            )
        }
    };
    haystack.to_lowercase().contains(needle_lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::persistence::types::{HistoryTrackKind, PersistedNavigationTrigger};

    #[test]
    fn filter_adapter_preserves_all_fields() {
        let filter = HistoryTimelineFilter {
            tracks: Some(vec![HistoryTrackKind::Traversal, HistoryTrackKind::NodeAudit]),
            node_id: Some("n1".to_string()),
            after_ms: Some(100),
            before_ms: Some(200),
            text_contains: Some("foo".to_string()),
        };
        let adapted = fact_filter_from_history_filter(&filter);
        let kinds = adapted.kinds.unwrap();
        assert!(kinds.contains(&ProjectedFactDiscriminant::Traversal));
        assert!(kinds.contains(&ProjectedFactDiscriminant::NodeAudit));
        assert!(!kinds.contains(&ProjectedFactDiscriminant::NodeNavigation));
        assert_eq!(adapted.node_id.as_deref(), Some("n1"));
        assert_eq!(adapted.after_ms, Some(100));
        assert_eq!(adapted.before_ms, Some(200));
        assert_eq!(adapted.text_contains.as_deref(), Some("foo"));
    }

    #[test]
    fn filter_adapter_handles_none_tracks() {
        let filter = HistoryTimelineFilter::default();
        let adapted = fact_filter_from_history_filter(&filter);
        assert!(adapted.kinds.is_none());
    }

    #[test]
    fn fact_to_timeline_event_roundtrips_traversal() {
        use crate::services::facts::types::{EventSourceRef, FactEnvelope, FactId};
        use uuid::Uuid;

        let fact = ProjectedFact {
            envelope: FactEnvelope {
                fact_id: FactId(Uuid::nil()),
                source: EventSourceRef { log_position: 5 },
                timestamp_ms: 999,
            },
            kind: ProjectedFactKind::Traversal {
                from_node_id: "a".to_string(),
                to_node_id: "b".to_string(),
                trigger: PersistedNavigationTrigger::LinkClick,
            },
        };
        let event = fact_to_timeline_event(&fact).unwrap();
        assert_eq!(event.timestamp_ms, 999);
        assert_eq!(event.log_position, 5);
        assert!(matches!(event.kind, HistoryEventKind::Traversal { .. }));
    }
}
