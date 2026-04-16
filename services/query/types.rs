/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Query input/output types for the `query` subsystem.
//!
//! Spec: `2026-03-18_fact_query_type_sketch.md §7`

use crate::services::facts::types::ProjectedFactDiscriminant;
use crate::services::persistence::types::{HistoryTimelineEvent, LogEntry};

/// Filter predicate applied inside [`GraphQueryEngine`].
///
/// All fields are optional; `None` = no constraint on that axis.
/// Multiple set fields are AND-combined.
#[derive(Debug, Clone, Default)]
pub struct FactQueryFilter {
    /// Restrict to specific fact families.
    pub kinds: Option<Vec<ProjectedFactDiscriminant>>,
    /// Include only facts referencing this node id.
    pub node_id: Option<String>,
    /// Include only facts with `timestamp_ms >= after_ms`.
    pub after_ms: Option<u64>,
    /// Include only facts with `timestamp_ms <= before_ms`.
    pub before_ms: Option<u64>,
    /// Case-insensitive substring match on the display text of the fact.
    pub text_contains: Option<String>,
}

/// A structured query over the [`FactStore`].
///
/// [`FactStore`]: crate::services::facts::FactStore
#[derive(Debug, Clone)]
pub enum GraphQuery {
    /// Mixed cross-track history timeline.
    MixedTimeline {
        filter: FactQueryFilter,
        limit: usize,
    },
    /// Per-node intra-node navigation history (NavigateNode entries).
    NodeNavigationHistory { node_id: String, limit: usize },
    /// Per-node metadata/lifecycle audit history.
    NodeAuditHistory { node_id: String, limit: usize },
}

/// Result type returned from [`GraphQueryEngine::execute`].
///
/// During the first migration slice, results adapt back to the existing
/// surface-facing types to avoid breaking active consumers.
///
/// [`GraphQueryEngine::execute`]: crate::services::query::GraphQueryEngine::execute
#[derive(Debug)]
pub enum GraphQueryResult {
    /// Result for `MixedTimeline` queries — existing surface type preserved.
    TimelineEvents(Vec<HistoryTimelineEvent>),
    /// Result for `NodeNavigationHistory` queries — raw `LogEntry` rows (existing surface type).
    NodeNavigationEntries(Vec<LogEntry>),
    /// Result for `NodeAuditHistory` queries — raw `LogEntry` rows (existing surface type).
    NodeAuditEntries(Vec<LogEntry>),
}

impl GraphQueryResult {
    /// Unwrap as `TimelineEvents`, panicking if the variant doesn't match.
    ///
    /// Use only in tests or adapter code that knows the query type.
    pub fn into_timeline_events(self) -> Vec<HistoryTimelineEvent> {
        match self {
            Self::TimelineEvents(v) => v,
            _ => panic!("GraphQueryResult: expected TimelineEvents"),
        }
    }

    pub fn into_node_navigation_entries(self) -> Vec<LogEntry> {
        match self {
            Self::NodeNavigationEntries(v) => v,
            _ => panic!("GraphQueryResult: expected NodeNavigationEntries"),
        }
    }

    pub fn into_node_audit_entries(self) -> Vec<LogEntry> {
        match self {
            Self::NodeAuditEntries(v) => v,
            _ => panic!("GraphQueryResult: expected NodeAuditEntries"),
        }
    }
}
