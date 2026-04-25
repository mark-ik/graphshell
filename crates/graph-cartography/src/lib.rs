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
pub const DEFAULT_CLUSTER_HYSTERESIS_MARGIN: f32 = 0.05;

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

    pub fn can_surface_in(self, destination: Self) -> bool {
        match destination {
            Self::LocalOnly => true,
            Self::DeviceSync => matches!(self, Self::DeviceSync | Self::Shared),
            Self::Shared => matches!(self, Self::Shared),
        }
    }

    pub fn requires_explicit_promotion_to(self, destination: Self) -> bool {
        !self.can_surface_in(destination)
    }

    pub fn can_surface_with_policy(self, policy: &CartographyPrivacyPolicy) -> bool {
        self.can_surface_in(policy.destination_scope)
            || policy
                .explicit_promotions
                .iter()
                .any(|promotion| promotion.allows(self, policy.destination_scope))
    }

    pub fn from_memory_privacy(privacy: EntryPrivacy) -> Self {
        match privacy {
            EntryPrivacy::LocalOnly => Self::LocalOnly,
            EntryPrivacy::ShareCandidate => Self::DeviceSync,
            EntryPrivacy::Shared => Self::Shared,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplicitPrivacyPromotion {
    pub from_scope: PrivacyScope,
    pub to_scope: PrivacyScope,
    pub authorized_at_ms: u64,
    pub rationale: String,
}

impl ExplicitPrivacyPromotion {
    pub fn allows(&self, from_scope: PrivacyScope, to_scope: PrivacyScope) -> bool {
        self.from_scope == from_scope
            && self.to_scope == to_scope
            && self
                .from_scope
                .requires_explicit_promotion_to(self.to_scope)
            && !self.rationale.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartographyPrivacyPolicy {
    pub destination_scope: PrivacyScope,
    pub explicit_promotions: Vec<ExplicitPrivacyPromotion>,
}

impl CartographyPrivacyPolicy {
    pub fn new(destination_scope: PrivacyScope) -> Self {
        Self {
            destination_scope,
            explicit_promotions: Vec::new(),
        }
    }

    pub fn with_explicit_promotion(mut self, promotion: ExplicitPrivacyPromotion) -> Self {
        self.explicit_promotions.push(promotion);
        self
    }

    pub fn can_surface(&self, scope: PrivacyScope) -> bool {
        scope.can_surface_with_policy(self)
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

impl StableClusterAssignmentSnapshot {
    pub fn hysteresis_decision_against(
        &self,
        candidate: &Self,
        confidence_margin: f32,
    ) -> HysteresisDecision {
        let shares_member = self
            .members
            .iter()
            .any(|entry| candidate.members.contains(entry));
        if !shares_member || self.cluster_id == candidate.cluster_id {
            return HysteresisDecision::Replace;
        }

        if candidate.confidence >= self.confidence + confidence_margin {
            HysteresisDecision::Replace
        } else {
            HysteresisDecision::KeepExisting
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HysteresisDecision {
    KeepExisting,
    Replace,
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
    EmptyAgentOutputProducer,
    EmptyAgentOutputVersion,
    EmptyAgentInvalidationVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeterministicAggregateKind {
    EntryEdgeRollups,
    ActivationFreshness,
    TraversalCentrality,
    RepeatedPathPriors,
    CoActivationPairs,
    FrameReformationPatterns,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LearnedAffinityCacheKind {
    StableClusterAssignments,
    TaskRegionMemberships,
    BridgeNodes,
    StableRelationPromotionCandidates,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubstrateMutationKind {
    VisitEntry,
    EnsureOwner,
    ReplaceLinearHistory,
    ResetOwner,
    DeleteOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphTruthMutationKind {
    AddNode,
    RemoveNode,
    ResetGraph,
    EdgeAssertion,
    TagChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalTimelineEventKind {
    NavigateNode,
    AppendNodeAuditEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    Active,
    Warm,
    Cold,
    Tombstone,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartographyInvalidationSignal {
    SubstrateMutation {
        kind: SubstrateMutationKind,
        owner: Option<WorkspaceOwner>,
        entry: Option<EntryKey>,
    },
    GraphTruthMutation {
        kind: GraphTruthMutationKind,
        node: Option<NodeKey>,
        entry: Option<EntryKey>,
    },
    WalTimelineEvent {
        kind: WalTimelineEventKind,
        entry: Option<EntryKey>,
    },
    SessionBoundary {
        session_bucket: String,
    },
    LifecycleTransition {
        state: LifecycleState,
        entry: Option<EntryKey>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartographyRuntimeInvalidationEvent {
    VisitEntry {
        owner: Option<WorkspaceOwner>,
        entry: Option<EntryKey>,
    },
    EnsureOwner {
        owner: WorkspaceOwner,
    },
    ReplaceLinearHistory {
        owner: Option<WorkspaceOwner>,
    },
    ResetOwner {
        owner: Option<WorkspaceOwner>,
    },
    DeleteOwner {
        owner: Option<WorkspaceOwner>,
    },
    GraphNodeAdded {
        node: NodeKey,
        entry: Option<EntryKey>,
    },
    GraphNodeRemoved {
        node: NodeKey,
        entry: Option<EntryKey>,
    },
    GraphReset,
    GraphEdgeAsserted {
        node: Option<NodeKey>,
        entry: Option<EntryKey>,
    },
    GraphTagChanged {
        node: NodeKey,
        entry: Option<EntryKey>,
    },
    WalNavigateNode {
        entry: Option<EntryKey>,
    },
    WalAppendNodeAuditEvent {
        entry: Option<EntryKey>,
    },
    SessionBoundary {
        session_bucket: String,
    },
    LifecycleActive {
        entry: Option<EntryKey>,
    },
    LifecycleWarm {
        entry: Option<EntryKey>,
    },
    LifecycleCold {
        entry: Option<EntryKey>,
    },
    LifecycleTombstone {
        entry: Option<EntryKey>,
    },
}

impl From<CartographyRuntimeInvalidationEvent> for CartographyInvalidationSignal {
    fn from(event: CartographyRuntimeInvalidationEvent) -> Self {
        match event {
            CartographyRuntimeInvalidationEvent::VisitEntry { owner, entry } => {
                Self::SubstrateMutation {
                    kind: SubstrateMutationKind::VisitEntry,
                    owner,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::EnsureOwner { owner } => Self::SubstrateMutation {
                kind: SubstrateMutationKind::EnsureOwner,
                owner: Some(owner),
                entry: None,
            },
            CartographyRuntimeInvalidationEvent::ReplaceLinearHistory { owner } => {
                Self::SubstrateMutation {
                    kind: SubstrateMutationKind::ReplaceLinearHistory,
                    owner,
                    entry: None,
                }
            }
            CartographyRuntimeInvalidationEvent::ResetOwner { owner } => Self::SubstrateMutation {
                kind: SubstrateMutationKind::ResetOwner,
                owner,
                entry: None,
            },
            CartographyRuntimeInvalidationEvent::DeleteOwner { owner } => Self::SubstrateMutation {
                kind: SubstrateMutationKind::DeleteOwner,
                owner,
                entry: None,
            },
            CartographyRuntimeInvalidationEvent::GraphNodeAdded { node, entry } => {
                Self::GraphTruthMutation {
                    kind: GraphTruthMutationKind::AddNode,
                    node: Some(node),
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::GraphNodeRemoved { node, entry } => {
                Self::GraphTruthMutation {
                    kind: GraphTruthMutationKind::RemoveNode,
                    node: Some(node),
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::GraphReset => Self::GraphTruthMutation {
                kind: GraphTruthMutationKind::ResetGraph,
                node: None,
                entry: None,
            },
            CartographyRuntimeInvalidationEvent::GraphEdgeAsserted { node, entry } => {
                Self::GraphTruthMutation {
                    kind: GraphTruthMutationKind::EdgeAssertion,
                    node,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::GraphTagChanged { node, entry } => {
                Self::GraphTruthMutation {
                    kind: GraphTruthMutationKind::TagChange,
                    node: Some(node),
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::WalNavigateNode { entry } => {
                Self::WalTimelineEvent {
                    kind: WalTimelineEventKind::NavigateNode,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::WalAppendNodeAuditEvent { entry } => {
                Self::WalTimelineEvent {
                    kind: WalTimelineEventKind::AppendNodeAuditEvent,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::SessionBoundary { session_bucket } => {
                Self::SessionBoundary { session_bucket }
            }
            CartographyRuntimeInvalidationEvent::LifecycleActive { entry } => {
                Self::LifecycleTransition {
                    state: LifecycleState::Active,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::LifecycleWarm { entry } => {
                Self::LifecycleTransition {
                    state: LifecycleState::Warm,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::LifecycleCold { entry } => {
                Self::LifecycleTransition {
                    state: LifecycleState::Cold,
                    entry,
                }
            }
            CartographyRuntimeInvalidationEvent::LifecycleTombstone { entry } => {
                Self::LifecycleTransition {
                    state: LifecycleState::Tombstone,
                    entry,
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartographyInvalidationEmission {
    pub signal: CartographyInvalidationSignal,
    pub plan: CartographyInvalidationPlan,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartographyInvalidationEmitter {
    pending: Vec<CartographyInvalidationEmission>,
}

impl CartographyInvalidationEmitter {
    pub fn emit_signal(
        &mut self,
        signal: CartographyInvalidationSignal,
    ) -> CartographyInvalidationEmission {
        let emission = CartographyInvalidationEmission::from_signal(signal);
        self.pending.push(emission.clone());
        emission
    }

    pub fn emit_runtime_event(
        &mut self,
        event: CartographyRuntimeInvalidationEvent,
    ) -> CartographyInvalidationEmission {
        self.emit_signal(event.into())
    }

    pub fn pending(&self) -> &[CartographyInvalidationEmission] {
        &self.pending
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn drain(&mut self) -> Vec<CartographyInvalidationEmission> {
        self.pending.drain(..).collect()
    }
}

impl CartographyInvalidationEmission {
    pub fn from_signal(signal: CartographyInvalidationSignal) -> Self {
        let plan = CartographyInvalidationPlan::from_signal(&signal);
        Self { signal, plan }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CartographyInvalidationPlan {
    pub deterministic: BTreeSet<DeterministicAggregateKind>,
    pub learned_affinity: BTreeSet<LearnedAffinityCacheKind>,
    pub full_recompute_allowed: bool,
}

impl CartographyInvalidationPlan {
    pub fn from_signals<'a, I>(signals: I) -> Self
    where
        I: IntoIterator<Item = &'a CartographyInvalidationSignal>,
    {
        let mut merged = Self::default();
        for signal in signals {
            merged.merge(Self::from_signal(signal));
        }
        merged
    }

    pub fn from_signal(signal: &CartographyInvalidationSignal) -> Self {
        let mut plan = Self::default();
        match signal {
            CartographyInvalidationSignal::SubstrateMutation { kind, .. } => match kind {
                SubstrateMutationKind::VisitEntry => {
                    plan.invalidate_deterministic([
                        DeterministicAggregateKind::EntryEdgeRollups,
                        DeterministicAggregateKind::ActivationFreshness,
                        DeterministicAggregateKind::TraversalCentrality,
                        DeterministicAggregateKind::RepeatedPathPriors,
                        DeterministicAggregateKind::CoActivationPairs,
                        DeterministicAggregateKind::FrameReformationPatterns,
                    ]);
                    plan.invalidate_learned_affinity([
                        LearnedAffinityCacheKind::StableClusterAssignments,
                        LearnedAffinityCacheKind::TaskRegionMemberships,
                        LearnedAffinityCacheKind::BridgeNodes,
                        LearnedAffinityCacheKind::StableRelationPromotionCandidates,
                    ]);
                }
                SubstrateMutationKind::EnsureOwner => {
                    plan.invalidate_deterministic([
                        DeterministicAggregateKind::CoActivationPairs,
                        DeterministicAggregateKind::FrameReformationPatterns,
                    ]);
                }
                SubstrateMutationKind::ReplaceLinearHistory
                | SubstrateMutationKind::ResetOwner
                | SubstrateMutationKind::DeleteOwner => {
                    plan.invalidate_all();
                }
            },
            CartographyInvalidationSignal::GraphTruthMutation { kind, .. } => {
                if matches!(kind, GraphTruthMutationKind::ResetGraph) {
                    plan.invalidate_all();
                } else {
                    plan.invalidate_deterministic([
                        DeterministicAggregateKind::ActivationFreshness,
                        DeterministicAggregateKind::EntryEdgeRollups,
                    ]);
                    plan.invalidate_learned_affinity([
                        LearnedAffinityCacheKind::StableClusterAssignments,
                        LearnedAffinityCacheKind::BridgeNodes,
                        LearnedAffinityCacheKind::StableRelationPromotionCandidates,
                    ]);
                }
            }
            CartographyInvalidationSignal::WalTimelineEvent { .. } => {
                plan.invalidate_deterministic([
                    DeterministicAggregateKind::ActivationFreshness,
                    DeterministicAggregateKind::TraversalCentrality,
                    DeterministicAggregateKind::RepeatedPathPriors,
                ]);
            }
            CartographyInvalidationSignal::SessionBoundary { .. } => {
                plan.invalidate_deterministic([
                    DeterministicAggregateKind::CoActivationPairs,
                    DeterministicAggregateKind::FrameReformationPatterns,
                ]);
            }
            CartographyInvalidationSignal::LifecycleTransition { .. } => {
                plan.invalidate_deterministic([DeterministicAggregateKind::ActivationFreshness]);
            }
        }
        plan
    }

    pub fn invalidate_all(&mut self) {
        self.invalidate_deterministic([
            DeterministicAggregateKind::EntryEdgeRollups,
            DeterministicAggregateKind::ActivationFreshness,
            DeterministicAggregateKind::TraversalCentrality,
            DeterministicAggregateKind::RepeatedPathPriors,
            DeterministicAggregateKind::CoActivationPairs,
            DeterministicAggregateKind::FrameReformationPatterns,
        ]);
        self.invalidate_learned_affinity([
            LearnedAffinityCacheKind::StableClusterAssignments,
            LearnedAffinityCacheKind::TaskRegionMemberships,
            LearnedAffinityCacheKind::BridgeNodes,
            LearnedAffinityCacheKind::StableRelationPromotionCandidates,
        ]);
        self.full_recompute_allowed = true;
    }

    pub fn merge(&mut self, other: Self) {
        self.deterministic.extend(other.deterministic);
        self.learned_affinity.extend(other.learned_affinity);
        self.full_recompute_allowed |= other.full_recompute_allowed;
    }

    fn invalidate_deterministic<I>(&mut self, kinds: I)
    where
        I: IntoIterator<Item = DeterministicAggregateKind>,
    {
        self.deterministic.extend(kinds);
    }

    fn invalidate_learned_affinity<I>(&mut self, kinds: I)
    where
        I: IntoIterator<Item = LearnedAffinityCacheKind>,
    {
        self.learned_affinity.extend(kinds);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeterministicAggregateCacheRecord {
    pub table_version: u32,
    pub built_at_ms: u64,
    pub tables: DeterministicAggregateTables,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LearnedAffinityCacheRecord {
    pub cache_version: u32,
    pub written_at_ms: u64,
    pub rows: LearnedAffinityCacheTables,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyPersistenceEnvelope {
    pub schema_version: u32,
    pub deterministic: DeterministicAggregateCacheRecord,
    pub learned_affinity: LearnedAffinityCacheRecord,
}

impl CartographyPersistenceEnvelope {
    pub fn from_snapshot(snapshot: CartographySnapshot, persisted_at_ms: u64) -> Self {
        Self {
            schema_version: snapshot.schema_version,
            deterministic: DeterministicAggregateCacheRecord {
                table_version: snapshot.deterministic_table_version,
                built_at_ms: snapshot.built_at_ms,
                tables: snapshot.deterministic,
            },
            learned_affinity: LearnedAffinityCacheRecord {
                cache_version: snapshot.learned_affinity_cache_version,
                written_at_ms: persisted_at_ms,
                rows: snapshot.learned_affinity,
            },
        }
    }

    pub fn validate(&self) -> Result<(), Vec<CartographySnapshotValidationError>> {
        self.clone().into_snapshot().validate()
    }

    pub fn into_snapshot(self) -> CartographySnapshot {
        CartographySnapshot {
            schema_version: self.schema_version,
            deterministic_table_version: self.deterministic.table_version,
            learned_affinity_cache_version: self.learned_affinity.cache_version,
            built_at_ms: self.deterministic.built_at_ms,
            deterministic: self.deterministic.tables,
            learned_affinity: self.learned_affinity.rows,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartographyPersistenceTrigger {
    Manual,
    InvalidationPlan,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyPersistenceWriteRequest {
    pub trigger: CartographyPersistenceTrigger,
    pub envelope: CartographyPersistenceEnvelope,
    pub invalidation_plan: Option<CartographyInvalidationPlan>,
}

impl CartographyPersistenceWriteRequest {
    pub fn from_snapshot(
        snapshot: CartographySnapshot,
        persisted_at_ms: u64,
        trigger: CartographyPersistenceTrigger,
    ) -> Self {
        Self {
            trigger,
            envelope: CartographyPersistenceEnvelope::from_snapshot(snapshot, persisted_at_ms),
            invalidation_plan: None,
        }
    }

    pub fn with_invalidation_plan(mut self, plan: CartographyInvalidationPlan) -> Self {
        self.invalidation_plan = Some(plan);
        self
    }

    pub fn validate(&self) -> Result<(), Vec<CartographySnapshotValidationError>> {
        self.envelope.validate()
    }
}

pub trait CartographyPersistenceSink {
    type Error;

    fn write_cartography_cache(
        &mut self,
        request: CartographyPersistenceWriteRequest,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InMemoryCartographyPersistenceSink {
    writes: Vec<CartographyPersistenceWriteRequest>,
}

impl InMemoryCartographyPersistenceSink {
    pub fn writes(&self) -> &[CartographyPersistenceWriteRequest] {
        &self.writes
    }

    pub fn drain(&mut self) -> Vec<CartographyPersistenceWriteRequest> {
        std::mem::take(&mut self.writes)
    }
}

impl CartographyPersistenceSink for InMemoryCartographyPersistenceSink {
    type Error = std::convert::Infallible;

    fn write_cartography_cache(
        &mut self,
        request: CartographyPersistenceWriteRequest,
    ) -> Result<(), Self::Error> {
        self.writes.push(request);
        Ok(())
    }
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyContributionAssemblyInput {
    pub schema_version: u32,
    pub built_at_ms: u64,
    pub destination_scope: PrivacyScope,
    pub edge_rollups: Vec<ContributionEdgeRollup>,
    pub relation_promotion_candidates: Vec<ContributionRelationPromotionCandidate>,
    pub privacy_scope: PrivacyScope,
}

impl CartographyContributionAssemblyInput {
    pub fn is_empty(&self) -> bool {
        self.edge_rollups.is_empty() && self.relation_promotion_candidates.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributionEdgeRollup {
    pub from_entry: EntryKey,
    pub to_entry: EntryKey,
    pub from_node: Option<NodeKey>,
    pub to_node: Option<NodeKey>,
    pub traversal_count: u64,
    pub latest_transition_at_ms: u64,
    pub transition_counts: HashMap<TransitionKind, u64>,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContributionRelationPromotionCandidate {
    pub from_entry: EntryKey,
    pub to_entry: EntryKey,
    pub from_node: Option<NodeKey>,
    pub to_node: Option<NodeKey>,
    pub confidence: f32,
    pub reason: String,
    pub version: String,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationPromotionTarget {
    AgentDerived,
    UserGroupedProposal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyRelationPromotionSurface {
    pub schema_version: u32,
    pub built_at_ms: u64,
    pub destination_scope: PrivacyScope,
    pub aggregate_evidence: Vec<CartographyRelationEvidence>,
    pub proposals: Vec<CartographyRelationPromotionProposal>,
    pub privacy_scope: PrivacyScope,
}

impl CartographyRelationPromotionSurface {
    pub fn is_empty(&self) -> bool {
        self.aggregate_evidence.is_empty() && self.proposals.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CartographyRelationEvidence {
    TraversalRollup {
        from_entry: EntryKey,
        to_entry: EntryKey,
        traversal_count: u64,
        latest_transition_at_ms: u64,
        privacy_scope: PrivacyScope,
    },
    CoActivation {
        a: EntryKey,
        b: EntryKey,
        count: u64,
        last_seen_at_ms: u64,
        decay_bucket: Option<String>,
        privacy_scope: PrivacyScope,
    },
}

impl CartographyRelationEvidence {
    pub fn privacy_scope(&self) -> PrivacyScope {
        match self {
            Self::TraversalRollup { privacy_scope, .. }
            | Self::CoActivation { privacy_scope, .. } => *privacy_scope,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyRelationPromotionProposal {
    pub from_entry: EntryKey,
    pub to_entry: EntryKey,
    pub from_node: Option<NodeKey>,
    pub to_node: Option<NodeKey>,
    pub target: RelationPromotionTarget,
    pub confidence: f32,
    pub reason: String,
    pub version: String,
    pub graph_intent: CartographyGraphIntentProposal,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CartographyGraphIntentProposal {
    ProposeAgentDerivedRelation {
        from_node: Option<NodeKey>,
        to_node: Option<NodeKey>,
        confidence: f32,
        reason: String,
        version: String,
    },
    ProposeUserGroupedRelation {
        from_node: Option<NodeKey>,
        to_node: Option<NodeKey>,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyAgentInputSurface {
    pub schema_version: u32,
    pub deterministic_table_version: u32,
    pub built_at_ms: u64,
    pub destination_scope: PrivacyScope,
    pub aggregates: DeterministicAggregateTables,
    pub privacy_scope: PrivacyScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartographyAgentOutputEnvelope {
    pub cache_version: u32,
    pub producer_id: String,
    pub output_version: String,
    pub invalidation_version: String,
    pub produced_at_ms: u64,
    pub rows: LearnedAffinityCacheTables,
    pub privacy_scope: PrivacyScope,
}

impl CartographyAgentOutputEnvelope {
    pub fn validate(&self) -> Result<(), Vec<CartographySnapshotValidationError>> {
        let mut errors = Vec::new();
        if self.cache_version != LEARNED_AFFINITY_CACHE_TABLE_VERSION {
            errors.push(
                CartographySnapshotValidationError::UnsupportedLearnedAffinityCacheVersion {
                    found: self.cache_version,
                    expected: LEARNED_AFFINITY_CACHE_TABLE_VERSION,
                },
            );
        }
        if self.producer_id.trim().is_empty() {
            errors.push(CartographySnapshotValidationError::EmptyAgentOutputProducer);
        }
        if self.output_version.trim().is_empty() {
            errors.push(CartographySnapshotValidationError::EmptyAgentOutputVersion);
        }
        if self.invalidation_version.trim().is_empty() {
            errors.push(CartographySnapshotValidationError::EmptyAgentInvalidationVersion);
        }
        if let Err(row_errors) = self.rows.validate() {
            errors.extend(row_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn into_cache_record(self) -> LearnedAffinityCacheRecord {
        LearnedAffinityCacheRecord {
            cache_version: self.cache_version,
            written_at_ms: self.produced_at_ms,
            rows: self.rows,
        }
    }
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

    pub fn contribution_assembly_input(
        &self,
        destination_scope: PrivacyScope,
    ) -> CartographyContributionAssemblyInput {
        let mut privacy_scope = PrivacyScope::Shared;

        let mut edge_rollups = Vec::new();
        for row in &self.deterministic.entry_edge_rollups {
            if !row.privacy_scope.can_surface_in(destination_scope) {
                continue;
            }
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            edge_rollups.push(ContributionEdgeRollup {
                from_entry: row.from_entry,
                to_entry: row.to_entry,
                from_node: self.graph_node_for_entry(row.from_entry),
                to_node: self.graph_node_for_entry(row.to_entry),
                traversal_count: row.traversal_count,
                latest_transition_at_ms: row.latest_transition_at_ms,
                transition_counts: row.transition_counts.clone(),
                privacy_scope: row.privacy_scope,
            });
        }

        let mut relation_promotion_candidates = Vec::new();
        for row in &self.learned_affinity.stable_relation_promotion_candidates {
            if !row.privacy_scope.can_surface_in(destination_scope) {
                continue;
            }
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            relation_promotion_candidates.push(ContributionRelationPromotionCandidate {
                from_entry: row.from_entry,
                to_entry: row.to_entry,
                from_node: self.graph_node_for_entry(row.from_entry),
                to_node: self.graph_node_for_entry(row.to_entry),
                confidence: row.confidence,
                reason: row.reason.clone(),
                version: row.version.clone(),
                privacy_scope: row.privacy_scope,
            });
        }

        CartographyContributionAssemblyInput {
            schema_version: self.schema_version,
            built_at_ms: self.built_at_ms,
            destination_scope,
            edge_rollups,
            relation_promotion_candidates,
            privacy_scope,
        }
    }

    pub fn relation_promotion_surface(
        &self,
        destination_scope: PrivacyScope,
    ) -> CartographyRelationPromotionSurface {
        self.relation_promotion_surface_with_policy(&CartographyPrivacyPolicy::new(
            destination_scope,
        ))
    }

    pub fn relation_promotion_surface_with_policy(
        &self,
        policy: &CartographyPrivacyPolicy,
    ) -> CartographyRelationPromotionSurface {
        let mut privacy_scope = PrivacyScope::Shared;
        let mut aggregate_evidence = Vec::new();
        let mut proposals = Vec::new();

        for row in &self.deterministic.entry_edge_rollups {
            if !policy.can_surface(row.privacy_scope) {
                continue;
            }
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            aggregate_evidence.push(CartographyRelationEvidence::TraversalRollup {
                from_entry: row.from_entry,
                to_entry: row.to_entry,
                traversal_count: row.traversal_count,
                latest_transition_at_ms: row.latest_transition_at_ms,
                privacy_scope: row.privacy_scope,
            });
        }

        for row in &self.deterministic.co_activation_pairs {
            if !policy.can_surface(row.privacy_scope) {
                continue;
            }
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            aggregate_evidence.push(CartographyRelationEvidence::CoActivation {
                a: row.a,
                b: row.b,
                count: row.count,
                last_seen_at_ms: row.last_seen_at_ms,
                decay_bucket: row.decay_bucket.clone(),
                privacy_scope: row.privacy_scope,
            });
        }

        for row in &self.learned_affinity.stable_relation_promotion_candidates {
            if row.from_entry == row.to_entry
                || row.reason.trim().is_empty()
                || !policy.can_surface(row.privacy_scope)
            {
                continue;
            }

            let from_node = self.graph_node_for_entry(row.from_entry);
            let to_node = self.graph_node_for_entry(row.to_entry);
            privacy_scope = privacy_scope.combine(row.privacy_scope);
            proposals.push(CartographyRelationPromotionProposal {
                from_entry: row.from_entry,
                to_entry: row.to_entry,
                from_node,
                to_node,
                target: RelationPromotionTarget::AgentDerived,
                confidence: row.confidence,
                reason: row.reason.clone(),
                version: row.version.clone(),
                graph_intent: CartographyGraphIntentProposal::ProposeAgentDerivedRelation {
                    from_node,
                    to_node,
                    confidence: row.confidence,
                    reason: row.reason.clone(),
                    version: row.version.clone(),
                },
                privacy_scope: row.privacy_scope,
            });
        }

        CartographyRelationPromotionSurface {
            schema_version: self.schema_version,
            built_at_ms: self.built_at_ms,
            destination_scope: policy.destination_scope,
            aggregate_evidence,
            proposals,
            privacy_scope,
        }
    }

    pub fn agent_input_surface(
        &self,
        destination_scope: PrivacyScope,
    ) -> CartographyAgentInputSurface {
        self.agent_input_surface_with_policy(&CartographyPrivacyPolicy::new(destination_scope))
    }

    pub fn agent_input_surface_with_policy(
        &self,
        policy: &CartographyPrivacyPolicy,
    ) -> CartographyAgentInputSurface {
        let mut privacy_scope = PrivacyScope::Shared;

        let entry_edge_rollups = self
            .deterministic
            .entry_edge_rollups
            .iter()
            .filter(|row| policy.can_surface(row.privacy_scope))
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                row.clone()
            })
            .collect();
        let activation_freshness = self
            .deterministic
            .activation_freshness
            .iter()
            .filter(|row| policy.can_surface(row.privacy_scope))
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                row.clone()
            })
            .collect();
        let traversal_centrality = self
            .deterministic
            .traversal_centrality
            .iter()
            .filter(|row| policy.can_surface(row.privacy_scope))
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                row.clone()
            })
            .collect();
        let repeated_path_priors = self
            .deterministic
            .repeated_path_priors
            .iter()
            .filter(|row| policy.can_surface(row.privacy_scope))
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                row.clone()
            })
            .collect();
        let co_activation_pairs = self
            .deterministic
            .co_activation_pairs
            .iter()
            .filter(|row| policy.can_surface(row.privacy_scope))
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                row.clone()
            })
            .collect();
        let frame_reformation_patterns = self
            .deterministic
            .frame_reformation_patterns
            .iter()
            .filter(|row| policy.can_surface(row.privacy_scope))
            .map(|row| {
                privacy_scope = privacy_scope.combine(row.privacy_scope);
                row.clone()
            })
            .collect();

        CartographyAgentInputSurface {
            schema_version: self.schema_version,
            deterministic_table_version: self.deterministic_table_version,
            built_at_ms: self.built_at_ms,
            destination_scope: policy.destination_scope,
            aggregates: DeterministicAggregateTables {
                entry_edge_rollups,
                activation_freshness,
                traversal_centrality,
                repeated_path_priors,
                co_activation_pairs,
                frame_reformation_patterns,
            },
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
    fn phase_one_workspace_memory_shares_entry_identity_and_visit_context() {
        let mut memory = WorkspaceGraphMemory::new();
        let graph_owner = memory.ensure_owner(WorkspaceOwner::GraphNode(NodeKey::new(7)), None);
        let pane_owner = memory.ensure_owner(WorkspaceOwner::Pane("pane-a".into()), None);
        let (key, payload) = entry(1, "https://a.test/");
        let payload = payload
            .with_graph_node(NodeKey::new(7))
            .with_content_fingerprint("sha256:abc");
        let graph_entry =
            memory.resolve_or_create_entry(key, payload.clone(), 10, EntryPrivacy::Shared);
        let pane_entry = memory.resolve_or_create_entry(key, payload, 20, EntryPrivacy::Shared);

        assert_eq!(graph_entry, pane_entry);

        memory
            .visit_entry(
                graph_owner,
                graph_entry,
                VisitContext::new(TransitionKind::UrlTyped)
                    .with_dwell_ms(25)
                    .with_session_bucket("session-a"),
                TransitionKind::UrlTyped,
                10,
            )
            .unwrap();
        memory
            .visit_entry(
                pane_owner,
                pane_entry,
                VisitContext::new(TransitionKind::LinkClick)
                    .with_referrer(key)
                    .with_dwell_ms(75)
                    .with_session_bucket("session-a"),
                TransitionKind::LinkClick,
                20,
            )
            .unwrap();

        let snapshot = CartographySnapshot::from_memory_at(&memory, 30);
        let activation = snapshot.activation_for_entry(key).unwrap();
        assert_eq!(activation.graph_node_id, Some(NodeKey::new(7)));
        assert_eq!(activation.revisit_count, 2);
        assert_eq!(activation.dwell_ms_total, 100);
        assert_eq!(activation.session_bucket.as_deref(), Some("session-a"));
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
    fn phase_two_keeps_deterministic_tables_and_learned_caches_split() {
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

        let learned = LearnedAffinityCacheTables {
            stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                cluster_id: "cluster-a".into(),
                members: vec![a_key, b_key],
                centroid_label: None,
                confidence: 0.8,
                version: "agent-v1".into(),
                last_recomputed_at_ms: 50,
                privacy_scope: PrivacyScope::Shared,
            }],
            ..LearnedAffinityCacheTables::default()
        };
        let snapshot =
            CartographySnapshot::from_memory_at(&memory, 60).with_learned_affinity(learned);

        assert_eq!(snapshot.deterministic.entry_edge_rollups.len(), 2);
        assert_eq!(snapshot.deterministic.activation_freshness.len(), 2);
        assert_eq!(snapshot.deterministic.repeated_path_priors.len(), 1);
        assert_eq!(
            snapshot.deterministic.repeated_path_priors[0].recurrence_count,
            2,
        );
        assert_eq!(
            snapshot.learned_affinity.stable_cluster_assignments.len(),
            1
        );
        assert!(snapshot.validate().is_ok());
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

    #[test]
    fn contribution_assembly_input_filters_rows_by_destination_scope() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
        let (c_key, c_payload) = entry(3, "https://c.test/");
        let a_node = NodeKey::new(10);
        let b_node = NodeKey::new(20);
        let c_node = NodeKey::new(30);
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
            EntryPrivacy::ShareCandidate,
        );
        let c = memory.resolve_or_create_entry(
            c_key,
            c_payload.with_graph_node(c_node),
            30,
            EntryPrivacy::LocalOnly,
        );

        for (entry_id, at_ms) in [(a, 10), (b, 20), (c, 30), (a, 40)] {
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

        let learned = LearnedAffinityCacheTables {
            stable_relation_promotion_candidates: vec![
                StableRelationPromotionCandidate {
                    from_entry: a_key,
                    to_entry: b_key,
                    confidence: 0.8,
                    reason: "device-safe relation".into(),
                    version: "agent-v4".into(),
                    privacy_scope: PrivacyScope::DeviceSync,
                },
                StableRelationPromotionCandidate {
                    from_entry: a_key,
                    to_entry: c_key,
                    confidence: 0.7,
                    reason: "local relation".into(),
                    version: "agent-v4".into(),
                    privacy_scope: PrivacyScope::LocalOnly,
                },
            ],
            ..LearnedAffinityCacheTables::default()
        };
        let snapshot =
            CartographySnapshot::from_memory_at(&memory, 70).with_learned_affinity(learned);

        let shared_input = snapshot.contribution_assembly_input(PrivacyScope::Shared);
        assert!(shared_input.is_empty());
        assert_eq!(
            shared_input.schema_version,
            CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION
        );
        assert_eq!(shared_input.built_at_ms, 70);
        assert_eq!(shared_input.destination_scope, PrivacyScope::Shared);
        assert_eq!(shared_input.privacy_scope, PrivacyScope::Shared);

        let device_input = snapshot.contribution_assembly_input(PrivacyScope::DeviceSync);
        assert!(!device_input.is_empty());
        assert_eq!(device_input.edge_rollups.len(), 1);
        assert_eq!(device_input.edge_rollups[0].from_entry, a_key);
        assert_eq!(device_input.edge_rollups[0].to_entry, b_key);
        assert_eq!(device_input.edge_rollups[0].from_node, Some(a_node));
        assert_eq!(device_input.edge_rollups[0].to_node, Some(b_node));
        assert_eq!(device_input.relation_promotion_candidates.len(), 1);
        assert_eq!(
            device_input.relation_promotion_candidates[0].reason,
            "device-safe relation",
        );
        assert_eq!(device_input.privacy_scope, PrivacyScope::DeviceSync);

        let local_input = snapshot.contribution_assembly_input(PrivacyScope::LocalOnly);
        assert_eq!(local_input.edge_rollups.len(), 3);
        assert_eq!(local_input.relation_promotion_candidates.len(), 2);
        assert_eq!(local_input.privacy_scope, PrivacyScope::LocalOnly);
    }

    #[test]
    fn phase_five_relation_surface_keeps_aggregates_as_evidence_not_edges() {
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
            EntryPrivacy::ShareCandidate,
        );

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

        let learned = LearnedAffinityCacheTables {
            stable_relation_promotion_candidates: vec![StableRelationPromotionCandidate {
                from_entry: a_key,
                to_entry: b_key,
                confidence: 0.82,
                reason: "stable bridge candidate".into(),
                version: "agent-v5".into(),
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            ..LearnedAffinityCacheTables::default()
        };
        let snapshot =
            CartographySnapshot::from_memory_at(&memory, 50).with_learned_affinity(learned);

        let surface = snapshot.relation_promotion_surface(PrivacyScope::DeviceSync);

        assert!(!surface.is_empty());
        assert!(surface.aggregate_evidence.iter().any(|evidence| matches!(
            evidence,
            CartographyRelationEvidence::TraversalRollup { from_entry, to_entry, .. }
                if *from_entry == a_key && *to_entry == b_key
        )));
        assert!(surface.aggregate_evidence.iter().any(|evidence| matches!(
            evidence,
            CartographyRelationEvidence::CoActivation { a, b, .. }
                if *a == a_key && *b == b_key
        )));
        assert_eq!(surface.proposals.len(), 1);
        assert_eq!(
            surface.proposals[0].target,
            RelationPromotionTarget::AgentDerived
        );
        assert!(matches!(
            surface.proposals[0].graph_intent,
            CartographyGraphIntentProposal::ProposeAgentDerivedRelation { .. }
        ));
        assert_eq!(surface.proposals[0].from_node, Some(a_node));
        assert_eq!(surface.proposals[0].to_node, Some(b_node));
        assert_eq!(surface.privacy_scope, PrivacyScope::DeviceSync);
    }

    #[test]
    fn phase_six_agent_surfaces_filter_inputs_and_ingest_outputs() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (a_key, a_payload) = entry(1, "https://a.test/");
        let (b_key, b_payload) = entry(2, "https://b.test/");
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

        let snapshot = CartographySnapshot::from_memory_at(&memory, 40);
        let input = snapshot.agent_input_surface(PrivacyScope::DeviceSync);
        assert_eq!(input.schema_version, CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(
            input.deterministic_table_version,
            DETERMINISTIC_AGGREGATE_TABLE_VERSION
        );
        assert_eq!(input.aggregates.activation_freshness.len(), 1);
        assert_eq!(input.aggregates.activation_freshness[0].entry, a_key);
        assert!(input.aggregates.entry_edge_rollups.is_empty());
        assert_eq!(input.privacy_scope, PrivacyScope::Shared);

        let rows = LearnedAffinityCacheTables {
            stable_cluster_assignments: vec![StableClusterAssignmentSnapshot {
                cluster_id: "cluster-a".into(),
                members: vec![a_key],
                centroid_label: Some("work".into()),
                confidence: 0.91,
                version: "agent-v6".into(),
                last_recomputed_at_ms: 50,
                privacy_scope: PrivacyScope::DeviceSync,
            }],
            ..LearnedAffinityCacheTables::default()
        };
        let output = CartographyAgentOutputEnvelope {
            cache_version: LEARNED_AFFINITY_CACHE_TABLE_VERSION,
            producer_id: "agent-registry".into(),
            output_version: "agent-v6".into(),
            invalidation_version: "gc-v1".into(),
            produced_at_ms: 60,
            rows: rows.clone(),
            privacy_scope: PrivacyScope::DeviceSync,
        };

        assert!(output.validate().is_ok());
        let record = output.into_cache_record();
        assert_eq!(record.cache_version, LEARNED_AFFINITY_CACHE_TABLE_VERSION);
        assert_eq!(record.written_at_ms, 60);
        assert_eq!(record.rows, rows);

        let invalid_output = CartographyAgentOutputEnvelope {
            cache_version: LEARNED_AFFINITY_CACHE_TABLE_VERSION,
            producer_id: " ".into(),
            output_version: "".into(),
            invalidation_version: " ".into(),
            produced_at_ms: 70,
            rows: LearnedAffinityCacheTables::default(),
            privacy_scope: PrivacyScope::Shared,
        };
        let errors = invalid_output
            .validate()
            .expect_err("agent output metadata should be required");
        assert!(errors.contains(&CartographySnapshotValidationError::EmptyAgentOutputProducer));
        assert!(errors.contains(&CartographySnapshotValidationError::EmptyAgentOutputVersion));
        assert!(
            errors.contains(&CartographySnapshotValidationError::EmptyAgentInvalidationVersion)
        );
    }

    #[test]
    fn phase_seven_privacy_policy_blocks_implicit_escalation() {
        let local = PrivacyScope::LocalOnly;
        assert!(local.requires_explicit_promotion_to(PrivacyScope::Shared));
        assert!(!CartographyPrivacyPolicy::new(PrivacyScope::Shared).can_surface(local));
        assert!(
            !CartographyPrivacyPolicy::new(PrivacyScope::Shared)
                .with_explicit_promotion(ExplicitPrivacyPromotion {
                    from_scope: PrivacyScope::LocalOnly,
                    to_scope: PrivacyScope::Shared,
                    authorized_at_ms: 10,
                    rationale: "   ".into(),
                })
                .can_surface(local)
        );
        assert!(
            CartographyPrivacyPolicy::new(PrivacyScope::Shared)
                .with_explicit_promotion(ExplicitPrivacyPromotion {
                    from_scope: PrivacyScope::LocalOnly,
                    to_scope: PrivacyScope::Shared,
                    authorized_at_ms: 20,
                    rationale: "user-approved contribution".into(),
                })
                .can_surface(local)
        );
    }

    #[test]
    fn phase_four_invalidation_plans_map_events_to_tables() {
        let visit_plan = CartographyInvalidationPlan::from_signal(
            &CartographyInvalidationSignal::SubstrateMutation {
                kind: SubstrateMutationKind::VisitEntry,
                owner: Some(WorkspaceOwner::Session("s1".into())),
                entry: Some(EntryKey::from_raw(1)),
            },
        );
        assert!(
            visit_plan
                .deterministic
                .contains(&DeterministicAggregateKind::ActivationFreshness)
        );
        assert!(
            visit_plan
                .deterministic
                .contains(&DeterministicAggregateKind::EntryEdgeRollups)
        );
        assert!(
            visit_plan
                .learned_affinity
                .contains(&LearnedAffinityCacheKind::StableClusterAssignments)
        );
        assert!(!visit_plan.full_recompute_allowed);

        let reset_plan = CartographyInvalidationPlan::from_signal(
            &CartographyInvalidationSignal::SubstrateMutation {
                kind: SubstrateMutationKind::ResetOwner,
                owner: None,
                entry: None,
            },
        );
        assert_eq!(reset_plan.deterministic.len(), 6);
        assert_eq!(reset_plan.learned_affinity.len(), 4);
        assert!(reset_plan.full_recompute_allowed);
    }

    #[test]
    fn follow_on_invalidation_emitter_records_signals_and_plans() {
        let mut emitter = CartographyInvalidationEmitter::default();
        assert!(emitter.is_empty());

        let emission =
            emitter.emit_runtime_event(CartographyRuntimeInvalidationEvent::VisitEntry {
                owner: Some(WorkspaceOwner::GraphNode(NodeKey::new(7))),
                entry: Some(EntryKey::from_raw(1)),
            });

        assert!(matches!(
            emission.signal,
            CartographyInvalidationSignal::SubstrateMutation {
                kind: SubstrateMutationKind::VisitEntry,
                ..
            }
        ));
        assert!(
            emission
                .plan
                .deterministic
                .contains(&DeterministicAggregateKind::ActivationFreshness)
        );
        assert!(
            emission
                .plan
                .learned_affinity
                .contains(&LearnedAffinityCacheKind::StableRelationPromotionCandidates)
        );
        assert_eq!(emitter.pending().len(), 1);

        let drained = emitter.drain();
        assert_eq!(drained, vec![emission]);
        assert!(emitter.is_empty());
    }

    #[test]
    fn follow_on_runtime_events_map_to_existing_invalidation_vocabulary() {
        let node = NodeKey::new(9);
        let entry = EntryKey::from_raw(3);

        let graph_signal: CartographyInvalidationSignal =
            CartographyRuntimeInvalidationEvent::GraphEdgeAsserted {
                node: Some(node),
                entry: Some(entry),
            }
            .into();
        assert_eq!(
            graph_signal,
            CartographyInvalidationSignal::GraphTruthMutation {
                kind: GraphTruthMutationKind::EdgeAssertion,
                node: Some(node),
                entry: Some(entry),
            }
        );

        let wal_signal: CartographyInvalidationSignal =
            CartographyRuntimeInvalidationEvent::WalNavigateNode { entry: Some(entry) }.into();
        assert_eq!(
            wal_signal,
            CartographyInvalidationSignal::WalTimelineEvent {
                kind: WalTimelineEventKind::NavigateNode,
                entry: Some(entry),
            }
        );

        let lifecycle_signal: CartographyInvalidationSignal =
            CartographyRuntimeInvalidationEvent::LifecycleTombstone { entry: Some(entry) }.into();
        assert_eq!(
            lifecycle_signal,
            CartographyInvalidationSignal::LifecycleTransition {
                state: LifecycleState::Tombstone,
                entry: Some(entry),
            }
        );
    }

    #[test]
    fn follow_on_invalidation_plans_merge_batched_runtime_signals() {
        let signals = [
            CartographyRuntimeInvalidationEvent::SessionBoundary {
                session_bucket: "session-a".into(),
            }
            .into(),
            CartographyRuntimeInvalidationEvent::ResetOwner { owner: None }.into(),
        ];

        let plan = CartographyInvalidationPlan::from_signals(signals.iter());

        assert_eq!(plan.deterministic.len(), 6);
        assert_eq!(plan.learned_affinity.len(), 4);
        assert!(plan.full_recompute_allowed);
        assert!(
            plan.deterministic
                .contains(&DeterministicAggregateKind::FrameReformationPatterns)
        );
    }

    #[test]
    fn phase_four_persistence_envelope_round_trips_snapshot_versions() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (entry_key, payload) = entry(1, "https://a.test/");
        let entry_id = memory.resolve_or_create_entry(entry_key, payload, 10, EntryPrivacy::Shared);
        memory
            .visit_entry(
                owner,
                entry_id,
                VisitContext::default().with_dwell_ms(10),
                TransitionKind::UrlTyped,
                10,
            )
            .unwrap();
        let snapshot = CartographySnapshot::from_memory_at(&memory, 20);

        let envelope = CartographyPersistenceEnvelope::from_snapshot(snapshot.clone(), 30);
        assert_eq!(envelope.schema_version, CARTOGRAPHY_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(
            envelope.deterministic.table_version,
            DETERMINISTIC_AGGREGATE_TABLE_VERSION
        );
        assert_eq!(
            envelope.learned_affinity.cache_version,
            LEARNED_AFFINITY_CACHE_TABLE_VERSION
        );
        assert_eq!(envelope.learned_affinity.written_at_ms, 30);
        assert!(envelope.validate().is_ok());
        assert_eq!(envelope.into_snapshot(), snapshot);
    }

    #[test]
    fn follow_on_persistence_sink_accepts_versioned_write_requests() {
        let mut memory = WorkspaceGraphMemory::new();
        let owner = memory.ensure_owner(WorkspaceOwner::Session("s1".into()), None);
        let (entry_key, payload) = entry(1, "https://a.test/");
        let entry_id = memory.resolve_or_create_entry(entry_key, payload, 10, EntryPrivacy::Shared);
        memory
            .visit_entry(
                owner,
                entry_id,
                VisitContext::default().with_dwell_ms(10),
                TransitionKind::UrlTyped,
                10,
            )
            .unwrap();

        let snapshot = CartographySnapshot::from_memory_at(&memory, 20);
        let request = CartographyPersistenceWriteRequest::from_snapshot(
            snapshot,
            30,
            CartographyPersistenceTrigger::InvalidationPlan,
        )
        .with_invalidation_plan(CartographyInvalidationPlan::from_signal(
            &CartographyInvalidationSignal::SessionBoundary {
                session_bucket: "s1".into(),
            },
        ));
        assert!(request.validate().is_ok());

        let mut sink = InMemoryCartographyPersistenceSink::default();
        sink.write_cartography_cache(request).unwrap();

        assert_eq!(sink.writes().len(), 1);
        assert_eq!(
            sink.writes()[0].trigger,
            CartographyPersistenceTrigger::InvalidationPlan
        );
        assert!(sink.writes()[0].invalidation_plan.is_some());
        assert_eq!(sink.drain().len(), 1);
        assert!(sink.writes().is_empty());
    }

    #[test]
    fn phase_four_cluster_hysteresis_blocks_low_confidence_reassignment() {
        let entry_key = EntryKey::from_raw(1);
        let existing = StableClusterAssignmentSnapshot {
            cluster_id: "cluster-a".into(),
            members: vec![entry_key],
            centroid_label: Some("alpha".into()),
            confidence: 0.80,
            version: "agent-v1".into(),
            last_recomputed_at_ms: 10,
            privacy_scope: PrivacyScope::Shared,
        };
        let weak_candidate = StableClusterAssignmentSnapshot {
            cluster_id: "cluster-b".into(),
            confidence: 0.83,
            version: "agent-v2".into(),
            last_recomputed_at_ms: 20,
            ..existing.clone()
        };
        let strong_candidate = StableClusterAssignmentSnapshot {
            confidence: 0.90,
            ..weak_candidate.clone()
        };

        assert_eq!(
            existing
                .hysteresis_decision_against(&weak_candidate, DEFAULT_CLUSTER_HYSTERESIS_MARGIN),
            HysteresisDecision::KeepExisting,
        );
        assert_eq!(
            existing
                .hysteresis_decision_against(&strong_candidate, DEFAULT_CLUSTER_HYSTERESIS_MARGIN),
            HysteresisDecision::Replace,
        );
    }
}
