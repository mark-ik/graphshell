// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Graph Cartography (GC).
//!
//! This crate is the workspace-scoped aggregate layer over `graph-memory`, graph
//! truth, and WAL/session signals. It starts with the contracts frozen in the
//! Graph Runtime Projection Layer plan: opaque entry identity, explicit privacy
//! scope, deterministic aggregate tables, and learned-affinity cache rows.

use std::collections::{BTreeSet, HashMap};

use graph_memory::{AggregatedEntryEdgeView, EntryId, EntryPrivacy, GraphMemory, TransitionKind};
use graphshell_core::graph::NodeKey;
use serde::{Deserialize, Serialize};

pub const CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const DETERMINISTIC_AGGREGATE_TABLE_VERSION: u32 = 1;
pub const LEARNED_AFFINITY_CACHE_TABLE_VERSION: u32 = 1;

/// Substrate-owned opaque identity for a graph-memory entry.
///
/// The runtime and GC layers use this handle instead of raw URLs. A separate
/// resolution row records the graph node, normalized locator, and optional
/// content fingerprint that back the handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryKey(u64);

impl EntryKey {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Resolution data backing an opaque [`EntryKey`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryResolution {
    pub graph_node_id: Option<NodeKey>,
    pub normalized_locator: String,
    pub content_fingerprint: Option<String>,
}

impl EntryResolution {
    pub fn new(normalized_locator: impl Into<String>) -> Self {
        Self {
            graph_node_id: None,
            normalized_locator: normalized_locator.into(),
            content_fingerprint: None,
        }
    }

    pub fn with_graph_node(mut self, graph_node_id: NodeKey) -> Self {
        self.graph_node_id = Some(graph_node_id);
        self
    }

    pub fn with_content_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.content_fingerprint = Some(fingerprint.into());
        self
    }
}

/// Visit-local context required by v1 cartographic aggregates.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VisitContext {
    pub transition: Option<TransitionKind>,
    pub referrer_entry: Option<EntryKey>,
    pub dwell_ms: Option<u64>,
    pub session_bucket: Option<String>,
}

impl VisitContext {
    pub fn new(transition: TransitionKind) -> Self {
        Self {
            transition: Some(transition),
            ..Self::default()
        }
    }

    pub fn with_referrer(mut self, referrer_entry: EntryKey) -> Self {
        self.referrer_entry = Some(referrer_entry);
        self
    }

    pub fn with_dwell_ms(mut self, dwell_ms: u64) -> Self {
        self.dwell_ms = Some(dwell_ms);
        self
    }

    pub fn with_session_bucket(mut self, session_bucket: impl Into<String>) -> Self {
        self.session_bucket = Some(session_bucket.into());
        self
    }
}

/// Workspace owner identity for the shared graph-memory fabric.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceOwner {
    GraphNode(NodeKey),
    GraphView(String),
    Pane(String),
    Session(String),
    Other(String),
}

pub type WorkspaceGraphMemory =
    GraphMemory<EntryKey, EntryResolution, WorkspaceOwner, VisitContext>;

/// Privacy scope declared at aggregate/cache birth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivacyScope {
    LocalOnly,
    DeviceSync,
    Shared,
}

impl PrivacyScope {
    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::LocalOnly, _) | (_, Self::LocalOnly) => Self::LocalOnly,
            (Self::DeviceSync, _) | (_, Self::DeviceSync) => Self::DeviceSync,
            (Self::Shared, Self::Shared) => Self::Shared,
        }
    }

    pub fn from_memory_privacy(privacy: EntryPrivacy) -> Self {
        match privacy {
            EntryPrivacy::LocalOnly => Self::LocalOnly,
            EntryPrivacy::ShareCandidate => Self::DeviceSync,
            EntryPrivacy::Shared => Self::Shared,
        }
    }
}

/// Deterministic elaboration of `AggregatedEntryEdgeView`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryEdgeRollup {
    pub from_entry: EntryKey,
    pub to_entry: EntryKey,
    pub traversal_count: u64,
    pub latest_transition_at_ms: u64,
    pub transition_counts: HashMap<TransitionKind, u64>,
    pub privacy_scope: PrivacyScope,
}

/// Last-activation and revisit/dwell aggregate per entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationFreshness {
    pub entry: EntryKey,
    pub graph_node_id: Option<NodeKey>,
    pub last_activation_at_ms: u64,
    pub revisit_count: u64,
    pub dwell_ms_total: u64,
    pub session_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

/// Branch-centrality summary used by importance scoring and bridge inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalCentrality {
    pub entry: EntryKey,
    pub inbound_count: u64,
    pub outbound_count: u64,
    pub bridge_count: u64,
    pub privacy_scope: PrivacyScope,
}

/// Recurring path prior over parent-child-grandchild chains.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepeatedPathPrior {
    pub path: Vec<EntryKey>,
    pub recurrence_count: u64,
    pub latest_seen_at_ms: u64,
    pub session_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

/// Nodes/entries active in the same session or time window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoActivationPair {
    pub a: EntryKey,
    pub b: EntryKey,
    pub count: u64,
    pub last_seen_at_ms: u64,
    pub decay_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

/// Recurring workbench frame membership pattern.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameReformationPattern {
    pub frame_signature: String,
    pub members: Vec<EntryKey>,
    pub recurrence_count: u64,
    pub latest_seen_at_ms: u64,
    pub privacy_scope: PrivacyScope,
}

/// Agent-produced stable cluster cache row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StableClusterAssignmentSnapshot {
    pub cluster_id: String,
    pub members: Vec<EntryKey>,
    pub centroid_label: Option<String>,
    pub confidence: f32,
    pub version: String,
    pub last_recomputed_at_ms: u64,
    pub privacy_scope: PrivacyScope,
}

/// Agent-produced task-region membership cache row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskRegionMembershipSnapshot {
    pub task_region_id: String,
    pub members: Vec<EntryKey>,
    pub confidence: f32,
    pub version: String,
    pub privacy_scope: PrivacyScope,
}

/// Agent-produced bridge annotation cache row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BridgeNodeSnapshot {
    pub entry: EntryKey,
    pub source_cluster_id: String,
    pub target_cluster_id: String,
    pub confidence: f32,
    pub invalidation_version: String,
    pub privacy_scope: PrivacyScope,
}

/// Agent-produced candidate for durable `AgentDerived` relation promotion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StableRelationPromotionCandidate {
    pub from_entry: EntryKey,
    pub to_entry: EntryKey,
    pub confidence: f32,
    pub reason: String,
    pub version: String,
    pub privacy_scope: PrivacyScope,
}

/// Learned-affinity cache table bundle.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct LearnedAffinityCacheTables {
    pub stable_cluster_assignments: Vec<StableClusterAssignmentSnapshot>,
    pub task_region_memberships: Vec<TaskRegionMembershipSnapshot>,
    pub bridge_nodes: Vec<BridgeNodeSnapshot>,
    pub stable_relation_promotion_candidates: Vec<StableRelationPromotionCandidate>,
}

impl LearnedAffinityCacheTables {
    pub fn is_empty(&self) -> bool {
        self.stable_cluster_assignments.is_empty()
            && self.task_region_memberships.is_empty()
            && self.bridge_nodes.is_empty()
            && self.stable_relation_promotion_candidates.is_empty()
    }

    pub fn stable_cluster_assignments_for_entry(
        &self,
        entry: EntryKey,
    ) -> Vec<&StableClusterAssignmentSnapshot> {
        self.stable_cluster_assignments
            .iter()
            .filter(|row| row.members.contains(&entry))
            .collect()
    }

    pub fn task_regions_for_entry(&self, entry: EntryKey) -> Vec<&TaskRegionMembershipSnapshot> {
        self.task_region_memberships
            .iter()
            .filter(|row| row.members.contains(&entry))
            .collect()
    }

    pub fn bridge_nodes_for_entry(&self, entry: EntryKey) -> Vec<&BridgeNodeSnapshot> {
        self.bridge_nodes
            .iter()
            .filter(|row| row.entry == entry)
            .collect()
    }

    pub fn relation_promotion_candidates_for_entry(
        &self,
        entry: EntryKey,
    ) -> Vec<&StableRelationPromotionCandidate> {
        self.stable_relation_promotion_candidates
            .iter()
            .filter(|row| row.from_entry == entry || row.to_entry == entry)
            .collect()
    }

    pub fn contains_entry(&self, entry: EntryKey) -> bool {
        !self.stable_cluster_assignments_for_entry(entry).is_empty()
            || !self.task_regions_for_entry(entry).is_empty()
            || !self.bridge_nodes_for_entry(entry).is_empty()
            || !self
                .relation_promotion_candidates_for_entry(entry)
                .is_empty()
    }

    pub fn versions_for_entry(&self, entry: EntryKey) -> Vec<String> {
        let mut versions = BTreeSet::new();
        for row in self.stable_cluster_assignments_for_entry(entry) {
            versions.insert(row.version.clone());
        }
        for row in self.task_regions_for_entry(entry) {
            versions.insert(row.version.clone());
        }
        for row in self.bridge_nodes_for_entry(entry) {
            versions.insert(row.invalidation_version.clone());
        }
        for row in self.relation_promotion_candidates_for_entry(entry) {
            versions.insert(row.version.clone());
        }
        versions.into_iter().collect()
    }

    pub fn has_rows_stale_for_version(&self, current_version: &str) -> bool {
        self.stable_cluster_assignments
            .iter()
            .any(|row| row.version != current_version)
            || self
                .task_region_memberships
                .iter()
                .any(|row| row.version != current_version)
            || self
                .bridge_nodes
                .iter()
                .any(|row| row.invalidation_version != current_version)
            || self
                .stable_relation_promotion_candidates
                .iter()
                .any(|row| row.version != current_version)
    }

    pub fn validate(&self) -> Result<(), Vec<CartographySnapshotValidationError>> {
        let mut errors = Vec::new();

        for row in &self.stable_cluster_assignments {
            if row.members.is_empty() {
                errors.push(
                    CartographySnapshotValidationError::EmptyStableClusterAssignment {
                        cluster_id: row.cluster_id.clone(),
                    },
                );
            }
        }

        for row in &self.task_region_memberships {
            if row.members.is_empty() {
                errors.push(
                    CartographySnapshotValidationError::EmptyTaskRegionMembership {
                        task_region_id: row.task_region_id.clone(),
                    },
                );
            }
        }

        for row in &self.stable_relation_promotion_candidates {
            if row.from_entry == row.to_entry {
                errors.push(
                    CartographySnapshotValidationError::SelfRelationPromotionCandidate {
                        entry: row.from_entry,
                    },
                );
            }
            if row.reason.trim().is_empty() {
                errors.push(
                    CartographySnapshotValidationError::EmptyRelationPromotionReason {
                        from_entry: row.from_entry,
                        to_entry: row.to_entry,
                    },
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Build v1 deterministic tables from a workspace-scoped graph-memory snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeterministicAggregateTables {
    pub entry_edge_rollups: Vec<EntryEdgeRollup>,
    pub activation_freshness: Vec<ActivationFreshness>,
    pub traversal_centrality: Vec<TraversalCentrality>,
    pub repeated_path_priors: Vec<RepeatedPathPrior>,
    pub co_activation_pairs: Vec<CoActivationPair>,
    pub frame_reformation_patterns: Vec<FrameReformationPattern>,
}

impl DeterministicAggregateTables {
    pub fn from_memory(memory: &WorkspaceGraphMemory) -> Self {
        Self {
            entry_edge_rollups: build_entry_edge_rollups(memory),
            activation_freshness: build_activation_freshness(memory),
            traversal_centrality: build_traversal_centrality(memory),
            repeated_path_priors: build_repeated_path_priors(memory),
            co_activation_pairs: build_co_activation_pairs(memory),
            frame_reformation_patterns: build_frame_reformation_patterns(memory),
        }
    }

    pub fn activation_for_entry(&self, entry: EntryKey) -> Option<&ActivationFreshness> {
        self.activation_freshness
            .iter()
            .find(|row| row.entry == entry)
    }

    pub fn centrality_for_entry(&self, entry: EntryKey) -> Option<&TraversalCentrality> {
        self.traversal_centrality
            .iter()
            .find(|row| row.entry == entry)
    }

    pub fn edge_rollups_from_entry(&self, entry: EntryKey) -> Vec<&EntryEdgeRollup> {
        self.entry_edge_rollups
            .iter()
            .filter(|row| row.from_entry == entry)
            .collect()
    }

    pub fn edge_rollups_to_entry(&self, entry: EntryKey) -> Vec<&EntryEdgeRollup> {
        self.entry_edge_rollups
            .iter()
            .filter(|row| row.to_entry == entry)
            .collect()
    }

    pub fn co_activation_pairs_for_entry(&self, entry: EntryKey) -> Vec<&CoActivationPair> {
        self.co_activation_pairs
            .iter()
            .filter(|row| row.a == entry || row.b == entry)
            .collect()
    }

    pub fn repeated_path_priors_containing_entry(
        &self,
        entry: EntryKey,
    ) -> Vec<&RepeatedPathPrior> {
        self.repeated_path_priors
            .iter()
            .filter(|row| row.path.contains(&entry))
            .collect()
    }

    pub fn frame_patterns_containing_entry(
        &self,
        entry: EntryKey,
    ) -> Vec<&FrameReformationPattern> {
        self.frame_reformation_patterns
            .iter()
            .filter(|row| row.members.contains(&entry))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartographySnapshotValidationError {
    UnsupportedSchemaVersion {
        found: u32,
        expected: u32,
    },
    UnsupportedDeterministicTableVersion {
        found: u32,
        expected: u32,
    },
    UnsupportedLearnedAffinityCacheVersion {
        found: u32,
        expected: u32,
    },
    EmptyStableClusterAssignment {
        cluster_id: String,
    },
    EmptyTaskRegionMembership {
        task_region_id: String,
    },
    SelfRelationPromotionCandidate {
        entry: EntryKey,
    },
    EmptyRelationPromotionReason {
        from_entry: EntryKey,
        to_entry: EntryKey,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyProjectionHints {
    pub entry: EntryKey,
    pub recency: Option<RecencyProjectionHint>,
    pub importance: Option<ImportanceProjectionHint>,
    pub parent_picker: Vec<ParentPickerHint>,
    pub annotations: Vec<NodeAnnotationHint>,
    pub privacy_scope: PrivacyScope,
}

impl CartographyProjectionHints {
    pub fn has_projection_data(&self) -> bool {
        self.recency.is_some()
            || self.importance.is_some()
            || !self.parent_picker.is_empty()
            || !self.annotations.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyProjectionHintSet {
    pub schema_version: u32,
    pub built_at_ms: u64,
    pub hints: Vec<CartographyProjectionHints>,
    pub missing_entries: Vec<EntryKey>,
    pub privacy_scope: PrivacyScope,
}

impl CartographyProjectionHintSet {
    pub fn is_empty(&self) -> bool {
        self.hints.is_empty() && self.missing_entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecencyProjectionHint {
    pub last_activation_at_ms: u64,
    pub revisit_count: u64,
    pub dwell_ms_total: u64,
    pub session_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportanceProjectionHint {
    pub inbound_count: u64,
    pub outbound_count: u64,
    pub bridge_count: u64,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentPickerHint {
    pub path: Vec<EntryKey>,
    pub recurrence_count: u64,
    pub latest_seen_at_ms: u64,
    pub session_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeAnnotationHint {
    pub source: CartographyAnnotationSource,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CartographyAnnotationSource {
    EdgeRollupOut {
        to_entry: EntryKey,
        traversal_count: u64,
        latest_transition_at_ms: u64,
    },
    EdgeRollupIn {
        from_entry: EntryKey,
        traversal_count: u64,
        latest_transition_at_ms: u64,
    },
    CoActivation {
        peer: EntryKey,
        count: u64,
        last_seen_at_ms: u64,
        decay_bucket: Option<String>,
    },
    FrameReformation {
        frame_signature: String,
        member_count: u64,
        recurrence_count: u64,
        latest_seen_at_ms: u64,
    },
    StableCluster {
        cluster_id: String,
        centroid_label: Option<String>,
        confidence: f32,
        version: String,
    },
    TaskRegion {
        task_region_id: String,
        confidence: f32,
        version: String,
    },
    BridgeNode {
        source_cluster_id: String,
        target_cluster_id: String,
        confidence: f32,
        invalidation_version: String,
    },
    StableRelationPromotion {
        peer: EntryKey,
        confidence: f32,
        reason: String,
        version: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartographyHistoryAnnotations {
    pub schema_version: u32,
    pub built_at_ms: u64,
    pub entries: Vec<HistoryEntryAnnotation>,
    pub privacy_scope: PrivacyScope,
}

impl CartographyHistoryAnnotations {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntryAnnotation {
    pub entry: EntryKey,
    pub graph_node_id: Option<NodeKey>,
    pub last_activation_at_ms: u64,
    pub revisit_count: u64,
    pub dwell_ms_total: u64,
    pub session_bucket: Option<String>,
    pub repeated_paths: Vec<HistoryRepeatedPathMarker>,
    pub co_activation_peers: Vec<HistoryCoActivationMarker>,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRepeatedPathMarker {
    pub path: Vec<EntryKey>,
    pub recurrence_count: u64,
    pub latest_seen_at_ms: u64,
    pub session_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryCoActivationMarker {
    pub peer: EntryKey,
    pub count: u64,
    pub last_seen_at_ms: u64,
    pub decay_bucket: Option<String>,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyCanvasSummary {
    pub schema_version: u32,
    pub built_at_ms: u64,
    pub edge_hotspots: Vec<CanvasEdgeHotspot>,
    pub activity_heat: Vec<CanvasActivityHeat>,
    pub cluster_halos: Vec<CanvasClusterHalo>,
    pub bridge_emphasis: Vec<CanvasBridgeEmphasis>,
    pub privacy_scope: PrivacyScope,
}

impl CartographyCanvasSummary {
    pub fn is_empty(&self) -> bool {
        self.edge_hotspots.is_empty()
            && self.activity_heat.is_empty()
            && self.cluster_halos.is_empty()
            && self.bridge_emphasis.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasEdgeHotspot {
    pub from_entry: EntryKey,
    pub to_entry: EntryKey,
    pub from_node: NodeKey,
    pub to_node: NodeKey,
    pub traversal_count: u64,
    pub latest_transition_at_ms: u64,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasActivityHeat {
    pub entry: EntryKey,
    pub node: NodeKey,
    pub last_activation_at_ms: u64,
    pub revisit_count: u64,
    pub dwell_ms_total: u64,
    pub heat_score: u64,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasClusterHalo {
    pub cluster_id: String,
    pub nodes: Vec<NodeKey>,
    pub centroid_label: Option<String>,
    pub confidence: f32,
    pub version: String,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasBridgeEmphasis {
    pub entry: EntryKey,
    pub node: NodeKey,
    pub source_cluster_id: String,
    pub target_cluster_id: String,
    pub confidence: f32,
    pub invalidation_version: String,
    pub privacy_scope: PrivacyScope,
}

/// Versioned GC handoff snapshot for consumers and cache persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographySnapshot {
    pub schema_version: u32,
    pub deterministic_table_version: u32,
    pub learned_affinity_cache_version: u32,
    pub built_at_ms: u64,
    pub deterministic: DeterministicAggregateTables,
    pub learned_affinity: LearnedAffinityCacheTables,
}

impl CartographySnapshot {
    pub fn from_memory_at(memory: &WorkspaceGraphMemory, built_at_ms: u64) -> Self {
        Self {
            schema_version: CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION,
            deterministic_table_version: DETERMINISTIC_AGGREGATE_TABLE_VERSION,
            learned_affinity_cache_version: LEARNED_AFFINITY_CACHE_TABLE_VERSION,
            built_at_ms,
            deterministic: DeterministicAggregateTables::from_memory(memory),
            learned_affinity: LearnedAffinityCacheTables::default(),
        }
    }

    pub fn with_learned_affinity(mut self, learned_affinity: LearnedAffinityCacheTables) -> Self {
        self.learned_affinity = learned_affinity;
        self
    }

    pub fn is_supported_version(&self) -> bool {
        self.schema_version == CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION
            && self.deterministic_table_version == DETERMINISTIC_AGGREGATE_TABLE_VERSION
            && self.learned_affinity_cache_version == LEARNED_AFFINITY_CACHE_TABLE_VERSION
    }

    pub fn validate(&self) -> Result<(), Vec<CartographySnapshotValidationError>> {
        let mut errors = Vec::new();

        if self.schema_version != CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION {
            errors.push(
                CartographySnapshotValidationError::UnsupportedSchemaVersion {
                    found: self.schema_version,
                    expected: CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION,
                },
            );
        }
        if self.deterministic_table_version != DETERMINISTIC_AGGREGATE_TABLE_VERSION {
            errors.push(
                CartographySnapshotValidationError::UnsupportedDeterministicTableVersion {
                    found: self.deterministic_table_version,
                    expected: DETERMINISTIC_AGGREGATE_TABLE_VERSION,
                },
            );
        }
        if self.learned_affinity_cache_version != LEARNED_AFFINITY_CACHE_TABLE_VERSION {
            errors.push(
                CartographySnapshotValidationError::UnsupportedLearnedAffinityCacheVersion {
                    found: self.learned_affinity_cache_version,
                    expected: LEARNED_AFFINITY_CACHE_TABLE_VERSION,
                },
            );
        }

        if let Err(cache_errors) = self.learned_affinity.validate() {
            errors.extend(cache_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn activation_for_entry(&self, entry: EntryKey) -> Option<&ActivationFreshness> {
        self.deterministic.activation_for_entry(entry)
    }

    pub fn centrality_for_entry(&self, entry: EntryKey) -> Option<&TraversalCentrality> {
        self.deterministic.centrality_for_entry(entry)
    }

    pub fn edge_rollups_from_entry(&self, entry: EntryKey) -> Vec<&EntryEdgeRollup> {
        self.deterministic.edge_rollups_from_entry(entry)
    }

    pub fn edge_rollups_to_entry(&self, entry: EntryKey) -> Vec<&EntryEdgeRollup> {
        self.deterministic.edge_rollups_to_entry(entry)
    }

    pub fn co_activation_pairs_for_entry(&self, entry: EntryKey) -> Vec<&CoActivationPair> {
        self.deterministic.co_activation_pairs_for_entry(entry)
    }

    pub fn repeated_path_priors_containing_entry(
        &self,
        entry: EntryKey,
    ) -> Vec<&RepeatedPathPrior> {
        self.deterministic
            .repeated_path_priors_containing_entry(entry)
    }

    pub fn frame_patterns_containing_entry(
        &self,
        entry: EntryKey,
    ) -> Vec<&FrameReformationPattern> {
        self.deterministic.frame_patterns_containing_entry(entry)
    }

    pub fn projection_hints_for_entry(&self, entry: EntryKey) -> CartographyProjectionHints {
        let recency = self
            .activation_for_entry(entry)
            .map(|row| RecencyProjectionHint {
                last_activation_at_ms: row.last_activation_at_ms,
                revisit_count: row.revisit_count,
                dwell_ms_total: row.dwell_ms_total,
                session_bucket: row.session_bucket.clone(),
                privacy_scope: row.privacy_scope,
            });
        let importance = self
            .centrality_for_entry(entry)
            .map(|row| ImportanceProjectionHint {
                inbound_count: row.inbound_count,
                outbound_count: row.outbound_count,
                bridge_count: row.bridge_count,
                privacy_scope: row.privacy_scope,
            });

        let mut privacy_scope = PrivacyScope::Shared;
        if let Some(row) = recency.as_ref() {
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        if let Some(row) = importance.as_ref() {
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }

        let mut parent_picker = Vec::new();
        for row in self.repeated_path_priors_containing_entry(entry) {
            parent_picker.push(ParentPickerHint {
                path: row.path.clone(),
                recurrence_count: row.recurrence_count,
                latest_seen_at_ms: row.latest_seen_at_ms,
                session_bucket: row.session_bucket.clone(),
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }

        let mut annotations = Vec::new();

        for row in self.edge_rollups_from_entry(entry) {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::EdgeRollupOut {
                    to_entry: row.to_entry,
                    traversal_count: row.traversal_count,
                    latest_transition_at_ms: row.latest_transition_at_ms,
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self.edge_rollups_to_entry(entry) {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::EdgeRollupIn {
                    from_entry: row.from_entry,
                    traversal_count: row.traversal_count,
                    latest_transition_at_ms: row.latest_transition_at_ms,
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self.co_activation_pairs_for_entry(entry) {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::CoActivation {
                    peer: if row.a == entry { row.b } else { row.a },
                    count: row.count,
                    last_seen_at_ms: row.last_seen_at_ms,
                    decay_bucket: row.decay_bucket.clone(),
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self.frame_patterns_containing_entry(entry) {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::FrameReformation {
                    frame_signature: row.frame_signature.clone(),
                    member_count: row.members.len() as u64,
                    recurrence_count: row.recurrence_count,
                    latest_seen_at_ms: row.latest_seen_at_ms,
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self
            .learned_affinity
            .stable_cluster_assignments_for_entry(entry)
        {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::StableCluster {
                    cluster_id: row.cluster_id.clone(),
                    centroid_label: row.centroid_label.clone(),
                    confidence: row.confidence,
                    version: row.version.clone(),
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self.learned_affinity.task_regions_for_entry(entry) {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::TaskRegion {
                    task_region_id: row.task_region_id.clone(),
                    confidence: row.confidence,
                    version: row.version.clone(),
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self.learned_affinity.bridge_nodes_for_entry(entry) {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::BridgeNode {
                    source_cluster_id: row.source_cluster_id.clone(),
                    target_cluster_id: row.target_cluster_id.clone(),
                    confidence: row.confidence,
                    invalidation_version: row.invalidation_version.clone(),
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }
        for row in self
            .learned_affinity
            .relation_promotion_candidates_for_entry(entry)
        {
            annotations.push(NodeAnnotationHint {
                source: CartographyAnnotationSource::StableRelationPromotion {
                    peer: if row.from_entry == entry {
                        row.to_entry
                    } else {
                        row.from_entry
                    },
                    confidence: row.confidence,
                    reason: row.reason.clone(),
                    version: row.version.clone(),
                },
                privacy_scope: row.privacy_scope,
            });
            privacy_scope = privacy_scope.combine(row.privacy_scope);
        }

        CartographyProjectionHints {
            entry,
            recency,
            importance,
            parent_picker,
            annotations,
            privacy_scope,
        }
    }

    pub fn projection_hints_for_entries<I>(&self, entries: I) -> CartographyProjectionHintSet
    where
        I: IntoIterator<Item = EntryKey>,
    {
        let mut seen = BTreeSet::new();
        let mut hints = Vec::new();
        let mut missing_entries = Vec::new();
        let mut privacy_scope = PrivacyScope::Shared;

        for entry in entries {
            if !seen.insert(entry) {
                continue;
            }

            let entry_hints = self.projection_hints_for_entry(entry);
            if entry_hints.has_projection_data() {
                privacy_scope = privacy_scope.combine(entry_hints.privacy_scope);
                hints.push(entry_hints);
            } else {
                missing_entries.push(entry);
            }
        }

        CartographyProjectionHintSet {
            schema_version: self.schema_version,
            built_at_ms: self.built_at_ms,
            hints,
            missing_entries,
            privacy_scope,
        }
    }

    pub fn history_annotations_for_entry(&self, entry: EntryKey) -> Option<HistoryEntryAnnotation> {
        let activation = self.activation_for_entry(entry)?;
        let mut privacy_scope = activation.privacy_scope;

        let repeated_paths = self
            .repeated_path_priors_containing_entry(entry)
            .into_iter()
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                HistoryRepeatedPathMarker {
                    path: row.path.clone(),
                    recurrence_count: row.recurrence_count,
                    latest_seen_at_ms: row.latest_seen_at_ms,
                    session_bucket: row.session_bucket.clone(),
                    privacy_scope: row.privacy_scope,
                }
            })
            .collect();

        let co_activation_peers = self
            .co_activation_pairs_for_entry(entry)
            .into_iter()
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                HistoryCoActivationMarker {
                    peer: if row.a == entry { row.b } else { row.a },
                    count: row.count,
                    last_seen_at_ms: row.last_seen_at_ms,
                    decay_bucket: row.decay_bucket.clone(),
                    privacy_scope: row.privacy_scope,
                }
            })
            .collect();

        Some(HistoryEntryAnnotation {
            entry,
            graph_node_id: activation.graph_node_id,
            last_activation_at_ms: activation.last_activation_at_ms,
            revisit_count: activation.revisit_count,
            dwell_ms_total: activation.dwell_ms_total,
            session_bucket: activation.session_bucket.clone(),
            repeated_paths,
            co_activation_peers,
            privacy_scope,
        })
    }

    pub fn history_annotations_for_entries<I>(&self, entries: I) -> CartographyHistoryAnnotations
    where
        I: IntoIterator<Item = EntryKey>,
    {
        let mut seen = BTreeSet::new();
        let mut annotations = Vec::new();
        let mut privacy_scope = PrivacyScope::Shared;

        for entry in entries {
            if !seen.insert(entry) {
                continue;
            }
            let Some(annotation) = self.history_annotations_for_entry(entry) else {
                continue;
            };
            privacy_scope = privacy_scope.combine(annotation.privacy_scope);
            annotations.push(annotation);
        }

        CartographyHistoryAnnotations {
            schema_version: self.schema_version,
            built_at_ms: self.built_at_ms,
            entries: annotations,
            privacy_scope,
        }
    }

    pub fn history_annotations(&self) -> CartographyHistoryAnnotations {
        self.history_annotations_for_entries(
            self.deterministic
                .activation_freshness
                .iter()
                .map(|row| row.entry),
        )
    }

    pub fn canvas_summary(&self) -> CartographyCanvasSummary {
        let mut privacy_scope = PrivacyScope::Shared;

        let mut edge_hotspots = Vec::new();
        for row in &self.deterministic.entry_edge_rollups {
            let (Some(from_node), Some(to_node)) = (
                self.graph_node_for_entry(row.from_entry),
                self.graph_node_for_entry(row.to_entry),
            ) else {
                continue;
            };
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            edge_hotspots.push(CanvasEdgeHotspot {
                from_entry: row.from_entry,
                to_entry: row.to_entry,
                from_node,
                to_node,
                traversal_count: row.traversal_count,
                latest_transition_at_ms: row.latest_transition_at_ms,
                privacy_scope: row.privacy_scope,
            });
        }

        let mut activity_heat = Vec::new();
        for row in &self.deterministic.activation_freshness {
            let Some(node) = row.graph_node_id else {
                continue;
            };
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            activity_heat.push(CanvasActivityHeat {
                entry: row.entry,
                node,
                last_activation_at_ms: row.last_activation_at_ms,
                revisit_count: row.revisit_count,
                dwell_ms_total: row.dwell_ms_total,
                heat_score: row.revisit_count.saturating_add(row.dwell_ms_total / 1_000),
                privacy_scope: row.privacy_scope,
            });
        }

        let mut cluster_halos = Vec::new();
        for row in &self.learned_affinity.stable_cluster_assignments {
            let mut nodes = row
                .members
                .iter()
                .filter_map(|entry| self.graph_node_for_entry(*entry))
                .collect::<Vec<_>>();
            nodes.sort();
            nodes.dedup();
            if nodes.is_empty() {
                continue;
            }
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            cluster_halos.push(CanvasClusterHalo {
                cluster_id: row.cluster_id.clone(),
                nodes,
                centroid_label: row.centroid_label.clone(),
                confidence: row.confidence,
                version: row.version.clone(),
                privacy_scope: row.privacy_scope,
            });
        }

        let mut bridge_emphasis = Vec::new();
        for row in &self.learned_affinity.bridge_nodes {
            let Some(node) = self.graph_node_for_entry(row.entry) else {
                continue;
            };
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            bridge_emphasis.push(CanvasBridgeEmphasis {
                entry: row.entry,
                node,
                source_cluster_id: row.source_cluster_id.clone(),
                target_cluster_id: row.target_cluster_id.clone(),
                confidence: row.confidence,
                invalidation_version: row.invalidation_version.clone(),
                privacy_scope: row.privacy_scope,
            });
        }

        CartographyCanvasSummary {
            schema_version: self.schema_version,
            built_at_ms: self.built_at_ms,
            edge_hotspots,
            activity_heat,
            cluster_halos,
            bridge_emphasis,
            privacy_scope,
        }
    }

    fn graph_node_for_entry(&self, entry: EntryKey) -> Option<NodeKey> {
        self.activation_for_entry(entry)?.graph_node_id
    }
}

pub fn build_entry_edge_rollups(memory: &WorkspaceGraphMemory) -> Vec<EntryEdgeRollup> {
    let mut rows = memory
        .aggregated_entry_edges()
        .into_iter()
        .filter_map(|edge| rollup_from_edge(memory, edge))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.from_entry, row.to_entry));
    rows
}

pub fn build_activation_freshness(memory: &WorkspaceGraphMemory) -> Vec<ActivationFreshness> {
    let mut dwell_by_entry: HashMap<EntryId, (u64, Option<String>)> = HashMap::new();
    for (_visit_id, visit) in memory.visits() {
        let entry = dwell_by_entry.entry(visit.entry).or_insert((0, None));
        entry.0 += visit.context.dwell_ms.unwrap_or(0);
        if let Some(bucket) = visit.context.session_bucket.as_ref() {
            entry.1 = Some(bucket.clone());
        }
    }

    let mut rows = memory
        .entries()
        .map(|(entry_id, entry)| {
            let (dwell_ms_total, session_bucket) =
                dwell_by_entry.remove(&entry_id).unwrap_or_default();
            ActivationFreshness {
                entry: entry.key,
                graph_node_id: entry.payload.graph_node_id,
                last_activation_at_ms: entry.last_seen_at_ms,
                revisit_count: entry.visit_count,
                dwell_ms_total,
                session_bucket,
                privacy_scope: PrivacyScope::from_memory_privacy(entry.privacy),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.entry);
    rows
}

pub fn build_traversal_centrality(memory: &WorkspaceGraphMemory) -> Vec<TraversalCentrality> {
    let mut counts: HashMap<EntryId, (u64, u64)> = HashMap::new();
    for edge in memory.edge_views() {
        counts.entry(edge.from_entry).or_default().1 += 1;
        counts.entry(edge.to_entry).or_default().0 += 1;
    }

    let mut rows = memory
        .entries()
        .map(|(entry_id, entry)| {
            let (inbound_count, outbound_count) = counts.remove(&entry_id).unwrap_or_default();
            TraversalCentrality {
                entry: entry.key,
                inbound_count,
                outbound_count,
                bridge_count: inbound_count.min(outbound_count),
                privacy_scope: PrivacyScope::from_memory_privacy(entry.privacy),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.entry);
    rows
}

pub fn build_repeated_path_priors(memory: &WorkspaceGraphMemory) -> Vec<RepeatedPathPrior> {
    let mut priors: HashMap<Vec<EntryKey>, RepeatedPathPrior> = HashMap::new();

    for (_owner_id, owner) in memory.owners() {
        let mut owned_visits = owner
            .owned_visits
            .iter()
            .filter_map(|visit_id| memory.visit(*visit_id).map(|visit| (*visit_id, visit)))
            .collect::<Vec<_>>();
        owned_visits.sort_by_key(|(_visit_id, visit)| visit.created_at_ms);

        for window in owned_visits.windows(2) {
            if let Some((path, at_ms, bucket, privacy_scope)) = path_row(memory, window) {
                update_path_prior(&mut priors, path, at_ms, bucket, privacy_scope);
            }
        }

        for window in owned_visits.windows(3) {
            if let Some((path, at_ms, bucket, privacy_scope)) = path_row(memory, window) {
                update_path_prior(&mut priors, path, at_ms, bucket, privacy_scope);
            }
        }
    }

    let mut rows = priors.into_values().collect::<Vec<_>>();
    rows.retain(|row| row.recurrence_count > 1);
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    rows
}

pub fn build_co_activation_pairs(memory: &WorkspaceGraphMemory) -> Vec<CoActivationPair> {
    let mut pairs: HashMap<(EntryKey, EntryKey), CoActivationPair> = HashMap::new();

    for cohort in visit_cohorts(memory) {
        for (left_index, a) in cohort.members.iter().enumerate() {
            for b in cohort.members.iter().skip(left_index + 1) {
                let key = (*a, *b);
                let row = pairs.entry(key).or_insert(CoActivationPair {
                    a: *a,
                    b: *b,
                    count: 0,
                    last_seen_at_ms: 0,
                    decay_bucket: None,
                    privacy_scope: PrivacyScope::Shared,
                });
                row.count += 1;
                row.last_seen_at_ms = row.last_seen_at_ms.max(cohort.latest_seen_at_ms);
                let pair_privacy = cohort
                    .member_privacy
                    .get(a)
                    .copied()
                    .unwrap_or(PrivacyScope::Shared)
                    .combine(
                        cohort
                            .member_privacy
                            .get(b)
                            .copied()
                            .unwrap_or(PrivacyScope::Shared),
                    );
                row.privacy_scope = row.privacy_scope.combine(pair_privacy);
                if cohort.session_bucket.is_some() {
                    row.decay_bucket = cohort.session_bucket.clone();
                }
            }
        }
    }

    let mut rows = pairs.into_values().collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.a, row.b));
    rows
}

pub fn build_frame_reformation_patterns(
    memory: &WorkspaceGraphMemory,
) -> Vec<FrameReformationPattern> {
    let mut patterns: HashMap<String, FrameReformationPattern> = HashMap::new();

    for cohort in visit_cohorts(memory) {
        let signature = frame_signature(&cohort.members);
        let row = patterns
            .entry(signature.clone())
            .or_insert(FrameReformationPattern {
                frame_signature: signature,
                members: cohort.members.clone(),
                recurrence_count: 0,
                latest_seen_at_ms: 0,
                privacy_scope: PrivacyScope::Shared,
            });
        row.recurrence_count += 1;
        row.latest_seen_at_ms = row.latest_seen_at_ms.max(cohort.latest_seen_at_ms);
        row.privacy_scope = row.privacy_scope.combine(cohort.privacy_scope);
    }

    let mut rows = patterns.into_values().collect::<Vec<_>>();
    rows.retain(|row| row.recurrence_count > 1);
    rows.sort_by(|a, b| a.frame_signature.cmp(&b.frame_signature));
    rows
}

#[derive(Clone, Debug)]
struct VisitCohort {
    members: Vec<EntryKey>,
    member_privacy: HashMap<EntryKey, PrivacyScope>,
    latest_seen_at_ms: u64,
    session_bucket: Option<String>,
    privacy_scope: PrivacyScope,
}

fn visit_cohorts(memory: &WorkspaceGraphMemory) -> Vec<VisitCohort> {
    let mut cohorts = Vec::new();

    for (_owner_id, owner) in memory.owners() {
        let mut visits_by_bucket: HashMap<Option<String>, Vec<graph_memory::VisitId>> =
            HashMap::new();
        for visit_id in &owner.owned_visits {
            let Some(visit) = memory.visit(*visit_id) else {
                continue;
            };
            visits_by_bucket
                .entry(visit.context.session_bucket.clone())
                .or_default()
                .push(*visit_id);
        }

        for (session_bucket, visit_ids) in visits_by_bucket {
            let mut members = Vec::new();
            let mut member_privacy: HashMap<EntryKey, PrivacyScope> = HashMap::new();
            let mut latest_seen_at_ms = 0;
            let mut privacy_scope = PrivacyScope::Shared;

            for visit_id in visit_ids {
                let Some(visit) = memory.visit(visit_id) else {
                    continue;
                };
                let Some(entry) = memory.entry(visit.entry) else {
                    continue;
                };
                members.push(entry.key);
                let entry_privacy = PrivacyScope::from_memory_privacy(entry.privacy);
                member_privacy
                    .entry(entry.key)
                    .and_modify(|privacy| *privacy = privacy.combine(entry_privacy))
                    .or_insert(entry_privacy);
                latest_seen_at_ms = latest_seen_at_ms.max(visit.created_at_ms);
                privacy_scope = privacy_scope.combine(entry_privacy);
            }

            members.sort();
            members.dedup();

            if members.len() > 1 {
                cohorts.push(VisitCohort {
                    members,
                    member_privacy,
                    latest_seen_at_ms,
                    session_bucket,
                    privacy_scope,
                });
            }
        }
    }

    cohorts.sort_by(|a, b| {
        a.members
            .cmp(&b.members)
            .then_with(|| a.session_bucket.cmp(&b.session_bucket))
            .then_with(|| a.latest_seen_at_ms.cmp(&b.latest_seen_at_ms))
    });
    cohorts
}

fn frame_signature(members: &[EntryKey]) -> String {
    members
        .iter()
        .map(|entry| entry.raw().to_string())
        .collect::<Vec<_>>()
        .join("+")
}

fn rollup_from_edge(
    memory: &WorkspaceGraphMemory,
    edge: AggregatedEntryEdgeView,
) -> Option<EntryEdgeRollup> {
    let from = memory.entry(edge.from_entry)?;
    let to = memory.entry(edge.to_entry)?;
    Some(EntryEdgeRollup {
        from_entry: from.key,
        to_entry: to.key,
        traversal_count: edge.traversal_count,
        latest_transition_at_ms: edge.latest_transition_at_ms,
        transition_counts: edge.transition_counts,
        privacy_scope: PrivacyScope::from_memory_privacy(from.privacy)
            .combine(PrivacyScope::from_memory_privacy(to.privacy)),
    })
}

fn path_row(
    memory: &WorkspaceGraphMemory,
    window: &[(
        graph_memory::VisitId,
        &graph_memory::VisitRecord<VisitContext>,
    )],
) -> Option<(Vec<EntryKey>, u64, Option<String>, PrivacyScope)> {
    let mut path = Vec::with_capacity(window.len());
    let mut privacy_scope = PrivacyScope::Shared;
    let mut latest_seen_at_ms = 0;
    let mut session_bucket = None;

    for (_visit_id, visit) in window {
        let entry = memory.entry(visit.entry)?;
        path.push(entry.key);
        privacy_scope = privacy_scope.combine(PrivacyScope::from_memory_privacy(entry.privacy));
        latest_seen_at_ms = latest_seen_at_ms.max(visit.created_at_ms);
        if let Some(bucket) = visit.context.session_bucket.as_ref() {
            session_bucket = Some(bucket.clone());
        }
    }

    Some((path, latest_seen_at_ms, session_bucket, privacy_scope))
}

fn update_path_prior(
    priors: &mut HashMap<Vec<EntryKey>, RepeatedPathPrior>,
    path: Vec<EntryKey>,
    at_ms: u64,
    session_bucket: Option<String>,
    privacy_scope: PrivacyScope,
) {
    let row = priors.entry(path.clone()).or_insert(RepeatedPathPrior {
        path,
        recurrence_count: 0,
        latest_seen_at_ms: 0,
        session_bucket: None,
        privacy_scope,
    });
    row.recurrence_count += 1;
    row.latest_seen_at_ms = row.latest_seen_at_ms.max(at_ms);
    row.privacy_scope = row.privacy_scope.combine(privacy_scope);
    if session_bucket.is_some() {
        row.session_bucket = session_bucket;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_memory::EntryPrivacy;

    fn entry(index: u64, locator: &str) -> (EntryKey, EntryResolution) {
        (EntryKey::from_raw(index), EntryResolution::new(locator))
    }

    #[test]
    fn entry_key_is_opaque_but_round_trippable() {
        let key = EntryKey::from_raw(42);
        assert_eq!(key.raw(), 42);
    }

    #[test]
    fn entry_edge_rollups_expose_entry_keys_and_privacy_scope() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::LocalOnly);

        memory
            .visit_entry(
                owner,
                a,
                VisitContext::new(TransitionKind::UrlTyped),
                TransitionKind::UrlTyped,
                10,
            )
            .unwrap();
        memory
            .visit_entry(
                owner,
                b,
                VisitContext::new(TransitionKind::LinkClick),
                TransitionKind::LinkClick,
                20,
            )
            .unwrap();

        let rows = build_entry_edge_rollups(&memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].from_entry, a_key);
        assert_eq!(rows[0].to_entry, b_key);
        assert_eq!(rows[0].traversal_count, 1);
        assert_eq!(rows[0].privacy_scope, PrivacyScope::LocalOnly);
        assert_eq!(
            rows[0].transition_counts.get(&TransitionKind::LinkClick),
            Some(&1),
        );
    }

    #[test]
    fn activation_freshness_accumulates_dwell_and_preserves_node_mapping() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::GraphNode(NodeKey::new(7)), None);
        let (key, payload) = entry(1, "https://a.test/");
        let payload = payload.with_graph_node(NodeKey::new(7));
        let entry_id = memory.resolve_or_create_entry(key, payload, 10, EntryPrivacy::Shared);

        memory
            .visit_entry(
                owner,
                entry_id,
                VisitContext::new(TransitionKind::UrlTyped)
                    .with_dwell_ms(50)
                    .with_session_bucket("session-a"),
                TransitionKind::UrlTyped,
                10,
            )
            .unwrap();
        memory
            .visit_entry(
                owner,
                entry_id,
                VisitContext::new(TransitionKind::Reload).with_dwell_ms(25),
                TransitionKind::Reload,
                30,
            )
            .unwrap();

        let rows = build_activation_freshness(&memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entry, key);
        assert_eq!(rows[0].graph_node_id, Some(NodeKey::new(7)));
        assert_eq!(rows[0].last_activation_at_ms, 30);
        assert_eq!(rows[0].revisit_count, 2);
        assert_eq!(rows[0].dwell_ms_total, 75);
        assert_eq!(rows[0].session_bucket.as_deref(), Some("session-a"));
    }

    #[test]
    fn traversal_centrality_counts_inbound_and_outbound_edges() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let (c_key, c_payload) = entry(3, "https://c.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::Shared);
        let c = memory.resolve_or_create_entry(c_key, c_payload, 30, EntryPrivacy::Shared);

        for (entry_id, at_ms) in [(a, 10), (b, 20), (c, 30)] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default(),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let rows = build_traversal_centrality(&memory);
        let b_row = rows.iter().find(|row| row.entry == b_key).unwrap();
        assert_eq!(b_row.inbound_count, 1);
        assert_eq!(b_row.outbound_count, 1);
        assert_eq!(b_row.bridge_count, 1);
    }

    #[test]
    fn repeated_path_priors_keep_only_recurring_paths() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::Shared);

        for (entry_id, at_ms) in [(a, 10), (b, 20), (a, 30), (b, 40)] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket("session-a"),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let rows = build_repeated_path_priors(&memory);
        let row = rows
            .iter()
            .find(|row| row.path == vec![a_key, b_key])
            .expect("a -> b should recur twice");
        assert_eq!(row.recurrence_count, 2);
        assert_eq!(row.latest_seen_at_ms, 40);
        assert_eq!(row.session_bucket.as_deref(), Some("session-a"));
    }

    #[test]
    fn co_activation_pairs_count_entries_in_same_session_bucket() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let (c_key, c_payload) = entry(3, "https://c.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::ShareCandidate);
        let c = memory.resolve_or_create_entry(c_key, c_payload, 30, EntryPrivacy::LocalOnly);

        for (entry_id, at_ms, bucket) in [
            (a, 10, "session-a"),
            (b, 20, "session-a"),
            (a, 30, "session-b"),
            (b, 40, "session-b"),
            (c, 50, "session-b"),
        ] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket(bucket),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let rows = build_co_activation_pairs(&memory);
        let ab = rows
            .iter()
            .find(|row| row.a == a_key && row.b == b_key)
            .expect("a and b share two buckets");
        assert_eq!(ab.count, 2);
        assert_eq!(ab.last_seen_at_ms, 50);
        assert_eq!(ab.decay_bucket.as_deref(), Some("session-b"));
        assert_eq!(ab.privacy_scope, PrivacyScope::DeviceSync);

        let bc = rows
            .iter()
            .find(|row| row.a == b_key && row.b == c_key)
            .expect("b and c share session-b");
        assert_eq!(bc.count, 1);
        assert_eq!(bc.privacy_scope, PrivacyScope::LocalOnly);
    }

    #[test]
    fn frame_reformation_patterns_keep_recurring_membership_sets() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner_a = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let owner_b = memory.ensure_owner(WorkspaceOwner::Session("s2".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let (c_key, c_payload) = entry(3, "https://c.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::Shared);
        let c = memory.resolve_or_create_entry(c_key, c_payload, 30, EntryPrivacy::Shared);

        for (owner, entry_id, at_ms, bucket) in [
            (owner_a, a, 10, "session-a"),
            (owner_a, b, 20, "session-a"),
            (owner_a, c, 30, "session-b"),
            (owner_b, b, 40, "session-c"),
            (owner_b, a, 50, "session-c"),
        ] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket(bucket),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let rows = build_frame_reformation_patterns(&memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].members, vec![a_key, b_key]);
        assert_eq!(rows[0].frame_signature, "1+2");
        assert_eq!(rows[0].recurrence_count, 2);
        assert_eq!(rows[0].latest_seen_at_ms, 50);
    }

    #[test]
    fn deterministic_tables_include_all_deterministic_builders() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner_a = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let owner_b = memory.ensure_owner(WorkspaceOwner::Session("s2".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::Shared);

        for (owner, entry_id, at_ms, bucket) in [
            (owner_a, a, 10, "session-a"),
            (owner_a, b, 20, "session-a"),
            (owner_b, a, 30, "session-b"),
            (owner_b, b, 40, "session-b"),
        ] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket(bucket),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let tables = DeterministicAggregateTables::from_memory(&memory);
        assert_eq!(tables.co_activation_pairs.len(), 1);
        assert_eq!(tables.frame_reformation_patterns.len(), 1);
    }

    #[test]
    fn cartography_snapshot_wraps_deterministic_tables_with_versions() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::Shared);

        for (entry_id, at_ms) in [(a, 10), (b, 20)] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket("session-a"),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let snapshot = CartographySnapshot::from_memory_at(&memory, 1234);
        assert!(snapshot.is_supported_version());
        assert_eq!(snapshot.schema_version, CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(snapshot.built_at_ms, 1234);
        assert_eq!(snapshot.deterministic.activation_freshness.len(), 2);
        assert_eq!(snapshot.deterministic.co_activation_pairs.len(), 1);
        assert!(snapshot.learned_affinity.is_empty());
    }

    #[test]
    fn cartography_snapshot_accepts_learned_affinity_cache_rows() {
        let memory = WorkspaceGraphMemory::new();
        let caches = LearnedAffinityCacheTables {
            stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                cluster_id: "cluster-a".into(),
                members: vec![EntryKey::from_raw(1), EntryKey::from_raw(2)],
                centroid_label: Some("work".into()),
                confidence: 0.8,
                version: "agent-v1".into(),
                last_recomputed_at_ms: 2000,
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            ..LearnedAffinityCacheTables::default()
        };

        let snapshot =
            CartographySnapshot::from_memory_at(&memory, 2100).with_learned_affinity(caches);

        assert!(snapshot.is_supported_version());
        assert!(!snapshot.learned_affinity.is_empty());
        assert_eq!(
            snapshot.learned_affinity.stable_cluster_assignments.len(),
            1
        );
        assert_eq!(
            snapshot.learned_affinity.stable_cluster_assignments[0].version,
            "agent-v1",
        );
    }

    #[test]
    fn snapshot_query_helpers_find_entry_rows() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner_a = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let owner_b = memory.ensure_owner(WorkspaceOwner::Session("s2".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::Shared);

        for (owner, entry_id, at_ms, bucket) in [
            (owner_a, a, 10, "session-a"),
            (owner_a, b, 20, "session-a"),
            (owner_b, a, 30, "session-b"),
            (owner_b, b, 40, "session-b"),
        ] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket(bucket),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let snapshot = CartographySnapshot::from_memory_at(&memory, 50);

        assert_eq!(
            snapshot.activation_for_entry(a_key).unwrap().revisit_count,
            2
        );
        assert_eq!(
            snapshot.centrality_for_entry(a_key).unwrap().outbound_count,
            2
        );
        assert_eq!(snapshot.edge_rollups_from_entry(a_key).len(), 1);
        assert_eq!(snapshot.edge_rollups_to_entry(b_key).len(), 1);
        assert_eq!(snapshot.co_activation_pairs_for_entry(a_key).len(), 1);
        assert_eq!(
            snapshot.repeated_path_priors_containing_entry(a_key).len(),
            1,
        );
        assert_eq!(snapshot.frame_patterns_containing_entry(a_key).len(), 1);
        assert!(
            snapshot
                .activation_for_entry(EntryKey::from_raw(999))
                .is_none()
        );
    }

    #[test]
    fn learned_cache_query_helpers_group_by_entry_and_version() {
        let a = EntryKey::from_raw(1);
        let b = EntryKey::from_raw(2);
        let c = EntryKey::from_raw(3);
        let caches = LearnedAffinityCacheTables {
            stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                cluster_id: "cluster-a".into(),
                members: vec![a, b],
                centroid_label: None,
                confidence: 0.9,
                version: "agent-v2".into(),
                last_recomputed_at_ms: 20,
                privacy_scope: PrivacyScope::Shared,
            }],
            task_region_memberships: vec![TaskRegionMembershipSnapshot {
                task_region_id: "task-a".into(),
                members: vec![a],
                confidence: 0.7,
                version: "agent-v2".into(),
                privacy_scope: PrivacyScope::Shared,
            }],
            bridge_nodes: vec![BridgeNodeSnapshot {
                entry: a,
                source_cluster_id: "cluster-a".into(),
                target_cluster_id: "cluster-b".into(),
                confidence: 0.6,
                invalidation_version: "agent-v2".into(),
                privacy_scope: PrivacyScope::Shared,
            }],
            stable_relation_promotion_candidates: vec![StableRelationPromotionCandidate {
                from_entry: b,
                to_entry: a,
                confidence: 0.8,
                reason: "recurring bridge".into(),
                version: "agent-v1".into(),
                privacy_scope: PrivacyScope::DeviceSync,
            }],
        };

        assert!(caches.contains_entry(a));
        assert!(!caches.contains_entry(c));
        assert_eq!(caches.stable_cluster_assignments_for_entry(a).len(), 1);
        assert_eq!(caches.task_regions_for_entry(a).len(), 1);
        assert_eq!(caches.bridge_nodes_for_entry(a).len(), 1);
        assert_eq!(caches.relation_promotion_candidates_for_entry(a).len(), 1);
        assert_eq!(
            caches.versions_for_entry(a),
            vec!["agent-v1".to_string(), "agent-v2".to_string()],
        );
        assert!(caches.has_rows_stale_for_version("agent-v2"));
        assert_eq!(caches.versions_for_entry(c), Vec::<String>::new());
    }

    #[test]
    fn snapshot_validate_reports_version_and_cache_errors() {
        let memory = WorkspaceGraphMemory::new();
        let entry = EntryKey::from_raw(1);
        let mut snapshot = CartographySnapshot::from_memory_at(&memory, 0).with_learned_affinity(
            LearnedAffinityCacheTables {
                stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                    cluster_id: "empty-cluster".into(),
                    members: Vec::new(),
                    centroid_label: None,
                    confidence: 0.3,
                    version: "agent-v1".into(),
                    last_recomputed_at_ms: 10,
                    privacy_scope: PrivacyScope::Shared,
                }],
                task_region_memberships: vec![TaskRegionMembershipSnapshot {
                    task_region_id: "empty-task".into(),
                    members: Vec::new(),
                    confidence: 0.4,
                    version: "agent-v1".into(),
                    privacy_scope: PrivacyScope::Shared,
                }],
                stable_relation_promotion_candidates: vec![StableRelationPromotionCandidate {
                    from_entry: entry,
                    to_entry: entry,
                    confidence: 0.5,
                    reason: "   ".into(),
                    version: "agent-v1".into(),
                    privacy_scope: PrivacyScope::Shared,
                }],
                ..LearnedAffinityCacheTables::default()
            },
        );
        snapshot.schema_version = 99;

        let errors = snapshot.validate().expect_err("snapshot should be invalid");
        assert!(errors.contains(
            &CartographySnapshotValidationError::UnsupportedSchemaVersion {
                found: 99,
                expected: CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION,
            }
        ));
        assert!(errors.contains(
            &CartographySnapshotValidationError::EmptyStableClusterAssignment {
                cluster_id: "empty-cluster".into(),
            }
        ));
        assert!(errors.contains(
            &CartographySnapshotValidationError::EmptyTaskRegionMembership {
                task_region_id: "empty-task".into(),
            }
        ));
        assert!(errors.contains(
            &CartographySnapshotValidationError::SelfRelationPromotionCandidate { entry },
        ));
        assert!(errors.contains(
            &CartographySnapshotValidationError::EmptyRelationPromotionReason {
                from_entry: entry,
                to_entry: entry,
            }
        ));
    }

    #[test]
    fn projection_hints_collect_scorer_parent_picker_and_annotation_inputs() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner_a = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let owner_b = memory.ensure_owner(WorkspaceOwner::Session("s2".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::ShareCandidate);

        for (owner, entry_id, at_ms, bucket) in [
            (owner_a, a, 10, "session-a"),
            (owner_a, b, 20, "session-a"),
            (owner_b, a, 30, "session-b"),
            (owner_b, b, 40, "session-b"),
        ] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default()
                        .with_session_bucket(bucket)
                        .with_dwell_ms(25),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let learned = LearnedAffinityCacheTables {
            stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                cluster_id: "cluster-a".into(),
                members: vec![a_key, b_key],
                centroid_label: Some("work".into()),
                confidence: 0.9,
                version: "agent-v2".into(),
                last_recomputed_at_ms: 50,
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            bridge_nodes: vec![BridgeNodeSnapshot {
                entry: a_key,
                source_cluster_id: "cluster-a".into(),
                target_cluster_id: "cluster-b".into(),
                confidence: 0.6,
                invalidation_version: "agent-v2".into(),
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            stable_relation_promotion_candidates: vec![StableRelationPromotionCandidate {
                from_entry: a_key,
                to_entry: b_key,
                confidence: 0.7,
                reason: "repeated transition".into(),
                version: "agent-v2".into(),
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            ..LearnedAffinityCacheTables::default()
        };
        let snapshot =
            CartographySnapshot::from_memory_at(&memory, 60).with_learned_affinity(learned);

        let hints = snapshot.projection_hints_for_entry(a_key);

        assert_eq!(hints.entry, a_key);
        assert_eq!(hints.recency.as_ref().unwrap().revisit_count, 2);
        assert_eq!(hints.recency.as_ref().unwrap().dwell_ms_total, 50);
        assert_eq!(hints.importance.as_ref().unwrap().outbound_count, 2);
        assert_eq!(hints.parent_picker.len(), 1);
        assert_eq!(hints.parent_picker[0].path, vec![a_key, b_key]);
        assert_eq!(hints.privacy_scope, PrivacyScope::DeviceSync);

        assert!(hints.annotations.iter().any(|hint| matches!(
            hint.source,
            CartographyAnnotationSource::EdgeRollupOut { to_entry, .. } if to_entry == b_key
        )));
        assert!(hints.annotations.iter().any(|hint| matches!(
            hint.source,
            CartographyAnnotationSource::CoActivation { peer, count: 2, .. } if peer == b_key
        )));
        assert!(hints.annotations.iter().any(|hint| matches!(
            hint.source,
            CartographyAnnotationSource::StableCluster { ref cluster_id, .. }
                if cluster_id == "cluster-a"
        )));
        assert!(hints.annotations.iter().any(|hint| matches!(
            hint.source,
            CartographyAnnotationSource::BridgeNode { ref target_cluster_id, .. }
                if target_cluster_id == "cluster-b"
        )));
        assert!(hints.annotations.iter().any(|hint| matches!(
            hint.source,
            CartographyAnnotationSource::StableRelationPromotion { peer, .. } if peer == b_key
        )));
    }

    #[test]
    fn projection_hint_set_preserves_scope_order_and_reports_missing_entries() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let missing_key = EntryKey::from_raw(99);
        let a = memory.resolve_or_create_entry(a_key, a_payload, 10, EntryPrivacy::Shared);
        let b = memory.resolve_or_create_entry(b_key, b_payload, 20, EntryPrivacy::LocalOnly);

        for (entry_id, at_ms) in [(a, 10), (b, 20), (a, 30)] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default().with_session_bucket("session-a"),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let snapshot = CartographySnapshot::from_memory_at(&memory, 123);
        let hint_set = snapshot.projection_hints_for_entries([b_key, missing_key, a_key, b_key]);

        assert!(!hint_set.is_empty());
        assert_eq!(hint_set.schema_version, CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(hint_set.built_at_ms, 123);
        assert_eq!(
            hint_set
                .hints
                .iter()
                .map(|hints| hints.entry)
                .collect::<Vec<_>>(),
            vec![b_key, a_key],
        );
        assert_eq!(hint_set.missing_entries, vec![missing_key]);
        assert_eq!(hint_set.privacy_scope, PrivacyScope::LocalOnly);
    }

    #[test]
    fn history_annotations_collect_activity_paths_and_co_activation() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner_a = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let owner_b = memory.ensure_owner(WorkspaceOwner::Session("s2".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a = memory.resolve_or_create_entry(
            a_key,
            a_payload.with_graph_node(NodeKey::new(10)),
            10,
            EntryPrivacy::Shared,
        );
        let b = memory.resolve_or_create_entry(
            b_key,
            b_payload.with_graph_node(NodeKey::new(20)),
            20,
            EntryPrivacy::ShareCandidate,
        );

        for (owner, entry_id, at_ms, bucket) in [
            (owner_a, a, 10, "session-a"),
            (owner_a, b, 20, "session-a"),
            (owner_b, a, 30, "session-b"),
            (owner_b, b, 40, "session-b"),
        ] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default()
                        .with_session_bucket(bucket)
                        .with_dwell_ms(1_500),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let snapshot = CartographySnapshot::from_memory_at(&memory, 50);
        let annotations = snapshot.history_annotations_for_entries([b_key, a_key, b_key]);

        assert!(!annotations.is_empty());
        assert_eq!(
            annotations.schema_version,
            CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION
        );
        assert_eq!(annotations.built_at_ms, 50);
        assert_eq!(annotations.privacy_scope, PrivacyScope::DeviceSync);
        assert_eq!(
            annotations
                .entries
                .iter()
                .map(|entry| entry.entry)
                .collect::<Vec<_>>(),
            vec![b_key, a_key],
        );

        let a_annotation = annotations
            .entries
            .iter()
            .find(|entry| entry.entry == a_key)
            .unwrap();
        assert_eq!(a_annotation.graph_node_id, Some(NodeKey::new(10)));
        assert_eq!(a_annotation.revisit_count, 2);
        assert_eq!(a_annotation.dwell_ms_total, 3_000);
        assert_eq!(a_annotation.repeated_paths.len(), 1);
        assert_eq!(a_annotation.repeated_paths[0].path, vec![a_key, b_key]);
        assert_eq!(a_annotation.co_activation_peers.len(), 1);
        assert_eq!(a_annotation.co_activation_peers[0].peer, b_key);
        assert_eq!(a_annotation.co_activation_peers[0].count, 2);
    }

    #[test]
    fn canvas_summary_collects_hotspots_activity_clusters_and_bridges() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let a_node = NodeKey::new(10);
        let b_node = NodeKey::new(20);
        let a = memory.resolve_or_create_entry(
            a_key,
            a_payload.with_graph_node(a_node),
            10,
            EntryPrivacy::Shared,
        );
        let b = memory.resolve_or_create_entry(
            b_key,
            b_payload.with_graph_node(b_node),
            20,
            EntryPrivacy::LocalOnly,
        );

        for (entry_id, at_ms) in [(a, 10), (b, 20), (a, 30)] {
            memory
                .visit_entry(
                    owner,
                    entry_id,
                    VisitContext::default()
                        .with_session_bucket("session-a")
                        .with_dwell_ms(2_000),
                    TransitionKind::LinkClick,
                    at_ms,
                )
                .unwrap();
        }

        let learned = LearnedAffinityCacheTables {
            stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                cluster_id: "cluster-a".into(),
                members: vec![b_key, a_key],
                centroid_label: Some("work".into()),
                confidence: 0.9,
                version: "agent-v3".into(),
                last_recomputed_at_ms: 40,
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            bridge_nodes: vec![BridgeNodeSnapshot {
                entry: b_key,
                source_cluster_id: "cluster-a".into(),
                target_cluster_id: "cluster-b".into(),
                confidence: 0.7,
                invalidation_version: "agent-v3".into(),
                privacy_scope: PrivacyScope::LocalOnly,
            }],
            ..LearnedAffinityCacheTables::default()
        };
        let snapshot =
            CartographySnapshot::from_memory_at(&memory, 60).with_learned_affinity(learned);

        let summary = snapshot.canvas_summary();

        assert!(!summary.is_empty());
        assert_eq!(summary.schema_version, CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(summary.built_at_ms, 60);
        assert_eq!(summary.privacy_scope, PrivacyScope::LocalOnly);
        assert_eq!(summary.edge_hotspots.len(), 2);
        assert!(summary.edge_hotspots.iter().any(|hotspot| {
            hotspot.from_node == a_node && hotspot.to_node == b_node && hotspot.traversal_count == 1
        }));
        assert_eq!(summary.activity_heat.len(), 2);
        let a_heat = summary
            .activity_heat
            .iter()
            .find(|heat| heat.entry == a_key)
            .unwrap();
        assert_eq!(a_heat.node, a_node);
        assert_eq!(a_heat.heat_score, 6);
        assert_eq!(summary.cluster_halos.len(), 1);
        assert_eq!(summary.cluster_halos[0].nodes, vec![a_node, b_node]);
        assert_eq!(summary.bridge_emphasis.len(), 1);
        assert_eq!(summary.bridge_emphasis[0].node, b_node);
    }
}
