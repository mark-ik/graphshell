/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Core types for the `fact_store` subsystem.
//!
//! Spec: `2026-03-18_event_log_fact_store_query_architecture.md §5`
//! Type sketch: `2026-03-18_fact_query_type_sketch.md §3–4`
//!
//! Design rules:
//! - All types are portable: no egui, Servo, or host-UI imports.
//! - `ProjectedFact` is a normalized read record, not a UI row.
//! - `FactEnvelope` provides temporal provenance back to the WAL.

use uuid::Uuid;

use crate::services::persistence::types::{NodeAuditEventKind, PersistedNavigationTrigger};

/// Opaque fact identity.
///
/// For deterministic rebuild, derived from `(log_position, per-entry ordinal)`
/// via [`FactId::from_position`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FactId(pub Uuid);

impl FactId {
    /// Construct a deterministic `FactId` from WAL position and ordinal.
    ///
    /// Ordinal disambiguates multiple facts projected from a single log entry
    /// (e.g. `AddNode` → both a `GraphStructure` and potentially a `NodeExists`
    /// fact in a future schema expansion).
    pub fn from_position(log_position: u64, ordinal: u8) -> Self {
        // Pack into the UUID v5 namespace to get stable, reproducible ids.
        // Using nil namespace and the raw bytes as name is acceptable for a
        // closed-world internal id scheme.
        let mut bytes = [0u8; 9];
        bytes[..8].copy_from_slice(&log_position.to_be_bytes());
        bytes[8] = ordinal;
        FactId(Uuid::new_v5(&Uuid::nil(), &bytes))
    }
}

/// Reference back to the originating WAL entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventSourceRef {
    pub log_position: u64,
}

/// Temporal envelope shared by all first-slice history facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactEnvelope {
    pub fact_id: FactId,
    pub source: EventSourceRef,
    pub timestamp_ms: u64,
}

/// Discriminant enum for kind-based indexing in [`FactStore`].
///
/// Mirrors `ProjectedFactKind` variants without carrying data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectedFactDiscriminant {
    Traversal,
    NodeNavigation,
    NodeAudit,
    GraphStructure,
}

/// The semantic payload of a projected fact.
///
/// Variants match the first-slice history event families:
/// `AppendTraversal`, `NavigateNode`, `AppendNodeAuditEvent`, `AddNode`, `RemoveNode`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectedFactKind {
    Traversal {
        from_node_id: String,
        to_node_id: String,
        trigger: PersistedNavigationTrigger,
    },
    NodeNavigation {
        node_id: String,
        from_url: String,
        to_url: String,
        trigger: PersistedNavigationTrigger,
    },
    NodeAudit {
        node_id: String,
        event: NodeAuditEventKind,
    },
    GraphStructure {
        node_id: String,
        /// `true` = node was added, `false` = node was removed.
        is_addition: bool,
    },
}

impl ProjectedFactKind {
    pub fn discriminant(&self) -> ProjectedFactDiscriminant {
        match self {
            Self::Traversal { .. } => ProjectedFactDiscriminant::Traversal,
            Self::NodeNavigation { .. } => ProjectedFactDiscriminant::NodeNavigation,
            Self::NodeAudit { .. } => ProjectedFactDiscriminant::NodeAudit,
            Self::GraphStructure { .. } => ProjectedFactDiscriminant::GraphStructure,
        }
    }

    /// Return the primary node id referenced by this fact, if any.
    pub fn primary_node_id(&self) -> Option<&str> {
        match self {
            Self::Traversal { from_node_id, .. } => Some(from_node_id.as_str()),
            Self::NodeNavigation { node_id, .. } => Some(node_id.as_str()),
            Self::NodeAudit { node_id, .. } => Some(node_id.as_str()),
            Self::GraphStructure { node_id, .. } => Some(node_id.as_str()),
        }
    }
}

/// A normalized projected read record derived from a WAL [`LogEntry`].
///
/// [`LogEntry`]: crate::services::persistence::types::LogEntry
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedFact {
    pub envelope: FactEnvelope,
    pub kind: ProjectedFactKind,
}

