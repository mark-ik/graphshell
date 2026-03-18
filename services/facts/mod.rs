/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! In-memory fact store for projected history facts.
//!
//! Spec: `2026-03-18_fact_query_type_sketch.md §6`
//!
//! Design rules:
//! - `facts` is the canonical ordered record; side indexes store `usize` positions only.
//! - Fully rebuilt from WAL at startup — no persistent materialization in this slice.
//! - UI code never calls `append_projected` directly; only the persistence layer does.

pub mod projector;
pub mod types;

use std::collections::HashMap;

use types::{ProjectedFact, ProjectedFactDiscriminant};

use crate::services::facts::projector::FactProjector;
use crate::services::persistence::types::LogEntry;

/// In-memory store of normalized projected facts.
///
/// Rebuilt from the WAL via [`FactStore::rebuild_from_log`] on startup.
/// Extended incrementally via [`FactStore::append_projected`] as new entries land.
pub struct FactStore {
    /// Canonical ordered list of projected facts (oldest → newest by WAL order).
    facts: Vec<ProjectedFact>,
    /// Index from node_id → positions in `facts`.
    by_node_id: HashMap<String, Vec<usize>>,
    /// Index from discriminant → positions in `facts`.
    by_kind: HashMap<ProjectedFactDiscriminant, Vec<usize>>,
}

impl FactStore {
    /// Construct an empty store.
    pub fn empty() -> Self {
        Self {
            facts: Vec::new(),
            by_node_id: HashMap::new(),
            by_kind: HashMap::new(),
        }
    }

    /// Rebuild the store by projecting every entry in `entries`.
    ///
    /// `entries` is an iterator of `(log_position, LogEntry)` pairs in WAL order.
    pub fn rebuild_from_log(
        projector: &impl FactProjector,
        entries: impl Iterator<Item = (u64, LogEntry)>,
    ) -> Self {
        let mut store = Self::empty();
        for (pos, entry) in entries {
            store.append_projected(projector, pos, &entry);
        }
        store
    }

    /// Project one WAL entry and append the resulting facts to the store.
    ///
    /// Called by the persistence layer after a new `LogEntry` is written.
    pub fn append_projected(
        &mut self,
        projector: &impl FactProjector,
        log_position: u64,
        entry: &LogEntry,
    ) {
        for fact in projector.project(log_position, entry) {
            let idx = self.facts.len();

            // Update node_id index
            if let Some(node_id) = fact.kind.primary_node_id() {
                self.by_node_id
                    .entry(node_id.to_string())
                    .or_default()
                    .push(idx);
            }
            // For Traversal, also index the *target* node
            if let types::ProjectedFactKind::Traversal { to_node_id, .. } = &fact.kind {
                self.by_node_id
                    .entry(to_node_id.clone())
                    .or_default()
                    .push(idx);
            }

            // Update discriminant index
            self.by_kind
                .entry(fact.kind.discriminant())
                .or_default()
                .push(idx);

            self.facts.push(fact);
        }
    }

    /// All projected facts, in WAL order (oldest first).
    pub fn facts(&self) -> &[ProjectedFact] {
        &self.facts
    }

    /// Facts indexed by node_id (positions in `facts()`).
    ///
    /// Returns an empty slice when `node_id` has no associated facts.
    pub(crate) fn positions_for_node(&self, node_id: &str) -> &[usize] {
        self.by_node_id.get(node_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Facts indexed by discriminant kind (positions in `facts()`).
    pub(crate) fn positions_for_kind(&self, kind: ProjectedFactDiscriminant) -> &[usize] {
        self.by_kind.get(&kind).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Number of stored facts.
    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::facts::projector::HistoryFactProjector;
    use crate::services::persistence::types::{NodeAuditEventKind, PersistedNavigationTrigger};

    fn projector() -> HistoryFactProjector {
        HistoryFactProjector
    }

    #[test]
    fn empty_store_has_no_facts() {
        let store = FactStore::empty();
        assert!(store.is_empty());
    }

    #[test]
    fn rebuild_from_empty_log_yields_empty_store() {
        let store = FactStore::rebuild_from_log(&projector(), std::iter::empty());
        assert!(store.is_empty());
    }

    #[test]
    fn append_traversal_indexes_both_nodes() {
        let mut store = FactStore::empty();
        store.append_projected(
            &projector(),
            1,
            &LogEntry::AppendTraversal {
                from_node_id: "a".to_string(),
                to_node_id: "b".to_string(),
                timestamp_ms: 100,
                trigger: PersistedNavigationTrigger::LinkClick,
            },
        );
        assert_eq!(store.len(), 1);
        assert_eq!(store.positions_for_node("a").len(), 1);
        assert_eq!(store.positions_for_node("b").len(), 1);
        assert_eq!(store.positions_for_node("c").len(), 0);
    }

    #[test]
    fn rebuild_from_log_projects_all_history_entries() {
        let entries = vec![
            (
                1,
                LogEntry::AddNode {
                    node_id: "n1".to_string(),
                    url: "https://a.test/".to_string(),
                    position_x: 0.0,
                    position_y: 0.0,
                    timestamp_ms: 1000,
                },
            ),
            (
                2,
                LogEntry::AppendNodeAuditEvent {
                    node_id: "n1".to_string(),
                    event: NodeAuditEventKind::Pinned,
                    timestamp_ms: 2000,
                },
            ),
            (
                3,
                // Non-history entry — should project nothing
                LogEntry::UpdateNodeTitle {
                    node_id: "n1".to_string(),
                    title: "New".to_string(),
                },
            ),
        ];
        let store = FactStore::rebuild_from_log(&projector(), entries.into_iter());
        assert_eq!(store.len(), 2); // AddNode + NodeAuditEvent; UpdateNodeTitle yields 0
        assert_eq!(store.positions_for_node("n1").len(), 2);
    }

    #[test]
    fn kind_index_groups_by_discriminant() {
        let entries = vec![
            (
                1,
                LogEntry::AppendTraversal {
                    from_node_id: "a".to_string(),
                    to_node_id: "b".to_string(),
                    timestamp_ms: 100,
                    trigger: PersistedNavigationTrigger::Unknown,
                },
            ),
            (
                2,
                LogEntry::AppendTraversal {
                    from_node_id: "b".to_string(),
                    to_node_id: "c".to_string(),
                    timestamp_ms: 200,
                    trigger: PersistedNavigationTrigger::Unknown,
                },
            ),
            (
                3,
                LogEntry::AddNode {
                    node_id: "x".to_string(),
                    url: "https://x.test/".to_string(),
                    position_x: 0.0,
                    position_y: 0.0,
                    timestamp_ms: 300,
                },
            ),
        ];
        let store = FactStore::rebuild_from_log(&projector(), entries.into_iter());
        assert_eq!(
            store.positions_for_kind(ProjectedFactDiscriminant::Traversal).len(),
            2
        );
        assert_eq!(
            store.positions_for_kind(ProjectedFactDiscriminant::GraphStructure).len(),
            1
        );
    }
}
