/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Serializable types for graph persistence.

use rkyv::{Archive, Deserialize, Serialize};

use crate::graph::{
    FrameLayoutHint, ImportRecord, NodeClassification, NodeImportProvenance,
    badge::NodeTagPresentationState,
};

/// Address type hint for persistence (mirrors `AddressKind` in the graph model).
///
/// Deprecated: superseded by [`PersistedAddress`]. Kept for rkyv backward compatibility
/// with old snapshots. No new values are written.
#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedAddressKind {
    #[default]
    Http,
    File,
    Data,
    GraphshellClip,
    Directory,
    Unknown,
}

/// Typed address for persistence — carries both the URL scheme classification
/// and the raw URL string. Mirrors [`crate::graph::Address`].
///
/// All variants store the full URL string so that [`PersistedAddress::as_url_str`]
/// is always a round-trip identity.
#[derive(
    Archive, Serialize, Deserialize, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedAddress {
    Http(String),
    File(String),
    Data(String),
    /// Clip route (`verso://clip/<id>` or `graphshell://clip/<id>`). Stores the full URL.
    Clip(String),
    Directory(String),
    Custom(String),
}

impl Default for PersistedAddress {
    /// Fallback used when deserializing old snapshots that lack the `address` field.
    /// The load path detects the empty URL and uses the legacy `url` field instead.
    fn default() -> Self {
        PersistedAddress::Custom(String::new())
    }
}

impl PersistedAddress {
    /// Return the raw URL string for this address.
    pub fn as_url_str(&self) -> &str {
        match self {
            PersistedAddress::Http(s)
            | PersistedAddress::File(s)
            | PersistedAddress::Data(s)
            | PersistedAddress::Clip(s)
            | PersistedAddress::Directory(s)
            | PersistedAddress::Custom(s) => s.as_str(),
        }
    }
}

impl ArchivedPersistedAddress {
    /// Return the raw URL string from the archived address.
    pub fn as_url_str(&self) -> &str {
        match self {
            ArchivedPersistedAddress::Http(s)
            | ArchivedPersistedAddress::File(s)
            | ArchivedPersistedAddress::Data(s)
            | ArchivedPersistedAddress::Clip(s)
            | ArchivedPersistedAddress::Directory(s)
            | ArchivedPersistedAddress::Custom(s) => s.as_str(),
        }
    }
}

/// Persisted per-node session fidelity state.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedNodeSessionState {
    pub history_entries: Vec<String>,
    pub history_index: usize,
    pub scroll_x: Option<f32>,
    pub scroll_y: Option<f32>,
    pub form_draft: Option<String>,
}

/// Persisted node.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedNode {
    /// Stable node identity.
    pub node_id: String,

    /// Typed address — canonical source of the node URL since Stage C.2.
    ///
    /// Old snapshots (pre-Stage C.2) will not have this field; `serde(default)`
    /// causes deserialization to use `PersistedAddress::Custom(String::new())`
    /// as the fallback. The load path in `from_snapshot` checks for an empty
    /// URL and falls back to the legacy `url` field in that case.
    #[serde(default)]
    pub address: PersistedAddress,

    /// Legacy URL field — written alongside `address` for backward compatibility
    /// with readers that predate Stage C.2. New readers prefer `address`.
    ///
    /// On old snapshots (no `address` field) this is the authoritative URL.
    #[serde(default)]
    pub url: String,

    #[serde(default)]
    pub cached_host: Option<String>,
    pub title: String,
    pub position_x: f32,
    pub position_y: f32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tag_presentation: NodeTagPresentationState,
    #[serde(default)]
    pub import_provenance: Vec<NodeImportProvenance>,
    pub is_pinned: bool,
    pub history_entries: Vec<String>,
    pub history_index: usize,
    pub thumbnail_png: Option<Vec<u8>>,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub favicon_rgba: Option<Vec<u8>>,
    pub favicon_width: u32,
    pub favicon_height: u32,
    pub session_state: Option<PersistedNodeSessionState>,
    /// Optional MIME type hint; drives renderer selection.
    pub mime_hint: Option<String>,
    /// Durable provenance-bearing classification records (Stage A enrichment).
    #[serde(default)]
    pub classifications: Vec<NodeClassification>,
    /// Durable split arrangement annotations for frame-anchor nodes.
    #[serde(default)]
    pub frame_layout_hints: Vec<FrameLayoutHint>,
    /// Durable split-offer suppression for frame-anchor nodes.
    #[serde(default)]
    pub frame_split_offer_suppressed: bool,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedEdgeFamily {
    Semantic,
    Traversal,
    Containment,
    Arrangement,
    Imported,
    Provenance,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedSemanticSubKind {
    Hyperlink,
    UserGrouped,
    AgentDerived,
    Cites,
    Quotes,
    Summarizes,
    Elaborates,
    ExampleOf,
    Supports,
    Contradicts,
    Questions,
    SameEntityAs,
    DuplicateOf,
    CanonicalMirrorOf,
    DependsOn,
    Blocks,
    NextStep,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedContainmentSubKind {
    UrlPath,
    Domain,
    FileSystem,
    UserFolder,
    ClipSource,
    NotebookSection,
    CollectionMember,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedArrangementSubKind {
    FrameMember,
    TileGroup,
    SplitPair,
    TabNeighbor,
    ActiveTab,
    PinnedInFrame,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedImportedSubKind {
    BookmarkFolder,
    HistoryImport,
    RssMembership,
    FileSystemImport,
    ArchiveMembership,
    SharedCollection,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedProvenanceSubKind {
    ClippedFrom,
    ExcerptedFrom,
    SummarizedFrom,
    TranslatedFrom,
    RewrittenFrom,
    GeneratedFrom,
    ExtractedFrom,
    ImportedFromSource,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedSemanticEdgeData {
    #[serde(default)]
    pub sub_kinds: Vec<PersistedSemanticSubKind>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub agent_decay_progress: Option<f32>,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedTraversalRecord {
    pub timestamp_ms: u64,
    pub trigger: PersistedNavigationTrigger,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedTraversalMetrics {
    pub total_navigations: u64,
    pub forward_navigations: u64,
    pub backward_navigations: u64,
    pub last_navigated_at: Option<u64>,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedTraversalEdgeData {
    #[serde(default)]
    pub traversals: Vec<PersistedTraversalRecord>,
    #[serde(default)]
    pub metrics: PersistedTraversalMetrics,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedContainmentEdgeData {
    #[serde(default)]
    pub sub_kinds: Vec<PersistedContainmentSubKind>,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedArrangementEdgeData {
    #[serde(default)]
    pub sub_kinds: Vec<PersistedArrangementSubKind>,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedImportedEdgeData {
    #[serde(default)]
    pub sub_kinds: Vec<PersistedImportedSubKind>,
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub struct PersistedProvenanceEdgeData {
    #[serde(default)]
    pub sub_kinds: Vec<PersistedProvenanceSubKind>,
}

#[derive(
    Archive, Serialize, Deserialize, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedEdgeAssertion {
    Semantic {
        sub_kind: PersistedSemanticSubKind,
        label: Option<String>,
        agent_decay_progress: Option<f32>,
    },
    Containment {
        sub_kind: PersistedContainmentSubKind,
    },
    Arrangement {
        sub_kind: PersistedArrangementSubKind,
    },
    Imported {
        sub_kind: PersistedImportedSubKind,
    },
    Provenance {
        sub_kind: PersistedProvenanceSubKind,
    },
}

#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedRelationSelector {
    Family(PersistedEdgeFamily),
    Semantic(PersistedSemanticSubKind),
    Containment(PersistedContainmentSubKind),
    Arrangement(PersistedArrangementSubKind),
    Imported(PersistedImportedSubKind),
    Provenance(PersistedProvenanceSubKind),
}

/// Persisted traversal trigger classification (v1 scope).
#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedNavigationTrigger {
    Unknown,
    LinkClick,
    Back,
    Forward,
    AddressBarEntry,
    PanePromotion,
    Programmatic,
}

/// The kind of node metadata or lifecycle event recorded in an audit log entry.
///
/// Each variant carries only the new value (not the old one). The sequence of
/// audit events in the WAL provides the full history; diffing adjacent entries
/// to recover the "from" value is a query-time operation.
#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug))]
pub enum NodeAuditEventKind {
    /// Node title was changed. Records the new title.
    TitleChanged { new_title: String },
    /// A tag was added to the node.
    Tagged { tag: String },
    /// A tag was removed from the node.
    Untagged { tag: String },
    /// Node was pinned.
    Pinned,
    /// Node was unpinned.
    Unpinned,
    /// Node URL was changed out-of-band (not via NavigateNode navigation).
    /// Used when a node's URL is set directly rather than through navigation.
    UrlChanged { new_url: String },
    /// Node was tombstoned (soft-deleted).
    Tombstoned,
    /// Node was restored from tombstone state.
    Restored,
}

/// Track-kind discriminant for filter predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryTrackKind {
    Traversal,
    NodeNavigation,
    NodeAudit,
    GraphStructure,
}

/// Typed event union for mixed-timeline queries. Each variant preserves
/// provenance — no synthetic coercion between tracks.
#[derive(Debug, Clone)]
pub enum HistoryEventKind {
    /// Inter-node traversal (TraversalHistory track).
    Traversal {
        from_node_id: String,
        to_node_id: String,
        trigger: PersistedNavigationTrigger,
    },
    /// Intra-node address evolution (NodeNavigationHistory track).
    NodeNavigation {
        node_id: String,
        from_url: String,
        to_url: String,
        trigger: PersistedNavigationTrigger,
    },
    /// Node metadata/lifecycle audit (NodeAuditHistory track).
    NodeAudit {
        node_id: String,
        event: NodeAuditEventKind,
    },
    /// Graph structural event: node added or removed.
    GraphStructure { node_id: String, is_addition: bool },
}

impl HistoryEventKind {
    pub fn track_kind(&self) -> HistoryTrackKind {
        match self {
            Self::Traversal { .. } => HistoryTrackKind::Traversal,
            Self::NodeNavigation { .. } => HistoryTrackKind::NodeNavigation,
            Self::NodeAudit { .. } => HistoryTrackKind::NodeAudit,
            Self::GraphStructure { .. } => HistoryTrackKind::GraphStructure,
        }
    }
}

/// Shared temporal envelope for every mixed-timeline row.
#[derive(Debug, Clone)]
pub struct HistoryTimelineEvent {
    /// Wall-clock time of the event (ms since UNIX epoch).
    pub timestamp_ms: u64,
    /// WAL log position for stable ordering of same-ms events.
    pub log_position: u64,
    /// The typed event payload.
    pub kind: HistoryEventKind,
}

/// Filter predicate for mixed-timeline queries. All fields are optional;
/// `None` means "no constraint on this axis." Multiple constraints are
/// AND-combined.
#[derive(Debug, Clone, Default)]
pub struct HistoryTimelineFilter {
    /// Include only these track kinds. `None` or empty = all tracks.
    pub tracks: Option<Vec<HistoryTrackKind>>,
    /// Include only events touching this node (as source, target, or subject).
    pub node_id: Option<String>,
    /// Include only events at or after this timestamp.
    pub after_ms: Option<u64>,
    /// Include only events at or before this timestamp.
    pub before_ms: Option<u64>,
    /// Full-text substring match on the event's display-text projection. Case-insensitive.
    pub text_contains: Option<String>,
}

/// Persisted edge.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedEdge {
    pub from_node_id: String,
    pub to_node_id: String,
    #[serde(default)]
    pub families: Vec<PersistedEdgeFamily>,
    #[serde(default)]
    pub semantic: Option<PersistedSemanticEdgeData>,
    #[serde(default)]
    pub traversal: Option<PersistedTraversalEdgeData>,
    #[serde(default)]
    pub containment: Option<PersistedContainmentEdgeData>,
    #[serde(default)]
    pub arrangement: Option<PersistedArrangementEdgeData>,
    #[serde(default)]
    pub imported: Option<PersistedImportedEdgeData>,
    #[serde(default)]
    pub provenance: Option<PersistedProvenanceEdgeData>,
}

/// Full graph snapshot for periodic saves.
#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct GraphSnapshot {
    pub nodes: Vec<PersistedNode>,
    pub edges: Vec<PersistedEdge>,
    pub import_records: Vec<ImportRecord>,
    pub timestamp_secs: u64,
}

/// Log entry for mutation journaling.
#[allow(deprecated)]
#[derive(Archive, Serialize, Deserialize, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LogEntry {
    AddNode {
        node_id: String,
        url: String,
        position_x: f32,
        position_y: f32,
        /// Wall-clock time of node creation (ms since UNIX epoch).
        timestamp_ms: u64,
    },
    AddEdge {
        from_node_id: String,
        to_node_id: String,
        assertion: PersistedEdgeAssertion,
    },
    RemoveEdge {
        from_node_id: String,
        to_node_id: String,
        selector: PersistedRelationSelector,
    },
    AppendTraversal {
        from_node_id: String,
        to_node_id: String,
        timestamp_ms: u64,
        trigger: PersistedNavigationTrigger,
    },
    UpdateNodeTitle {
        node_id: String,
        title: String,
    },
    PinNode {
        node_id: String,
        is_pinned: bool,
    },
    RemoveNode {
        node_id: String,
        /// Wall-clock time of node removal (ms since UNIX epoch).
        timestamp_ms: u64,
    },
    ClearGraph,
    UpdateNodeUrl {
        node_id: String,
        new_url: String,
    },
    TagNode {
        node_id: String,
        tag: String,
    },
    UntagNode {
        node_id: String,
        tag: String,
    },
    /// Set (or clear) the MIME type hint on a node.
    UpdateNodeMimeHint {
        node_id: String,
        /// `None` clears the hint; `Some(mime)` sets it.
        mime_hint: Option<String>,
    },
    /// Set (or clear) the durable viewer override on a node.
    UpdateNodeViewerOverride {
        node_id: String,
        /// `None` clears the override (automatic selection); `Some(viewer_id)` forces a viewer.
        viewer_override: Option<String>,
    },
    /// Append a durable split arrangement hint to a frame anchor.
    RecordFrameLayoutHint {
        frame_id: String,
        hint: FrameLayoutHint,
    },
    /// Remove a durable split arrangement hint from a frame anchor by index.
    RemoveFrameLayoutHint {
        frame_id: String,
        hint_index: usize,
    },
    /// Reorder a durable split arrangement hint within a frame anchor.
    MoveFrameLayoutHint {
        frame_id: String,
        from_index: usize,
        to_index: usize,
    },
    /// Persist per-frame split-offer suppression.
    SetFrameSplitOfferSuppressed {
        frame_id: String,
        suppressed: bool,
    },
    /// Record a within-node URL navigation (same node, new address).
    /// Emitted alongside `UpdateNodeUrl` to preserve the from→to transition
    /// in the WAL. Unlike `AppendTraversal` (which records inter-node movement),
    /// this records intra-node address evolution.
    NavigateNode {
        node_id: String,
        from_url: String,
        to_url: String,
        trigger: PersistedNavigationTrigger,
        /// Wall-clock time of the navigation (ms since UNIX epoch).
        timestamp_ms: u64,
    },
    /// Append a metadata or lifecycle audit event for a node.
    /// Emitted alongside the existing snapshot entries (`UpdateNodeTitle`,
    /// `TagNode`, etc.) which remain for WAL replay. This entry provides the
    /// timestamped audit trail that snapshot entries lack.
    AppendNodeAuditEvent {
        node_id: String,
        event: NodeAuditEventKind,
        /// Wall-clock time of the event (ms since UNIX epoch).
        timestamp_ms: u64,
    },
    /// Assign a classification record to a node.
    AssignClassification {
        node_id: String,
        classification: NodeClassification,
    },
    /// Remove a classification record from a node by scheme + value.
    UnassignClassification {
        node_id: String,
        scheme: crate::graph::ClassificationScheme,
        value: String,
    },
    /// Update the status of a classification record on a node.
    UpdateClassificationStatus {
        node_id: String,
        scheme: crate::graph::ClassificationScheme,
        value: String,
        status: crate::graph::ClassificationStatus,
    },
    /// Mark a classification as the primary one for its scheme.
    SetPrimaryClassification {
        node_id: String,
        scheme: crate::graph::ClassificationScheme,
        value: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_persisted_node_roundtrip() {
        let node = PersistedNode {
            node_id: Uuid::new_v4().to_string(),
            address: PersistedAddress::Http("https://example.com".to_string()),
            url: "https://example.com".to_string(),
            cached_host: Some("example.com".to_string()),
            title: "Example".to_string(),
            position_x: 100.0,
            position_y: 200.0,
            tags: vec!["udc:51".to_string(), "#pin".to_string()],
            tag_presentation: NodeTagPresentationState::default(),
            import_provenance: vec![NodeImportProvenance {
                source_id: "import:firefox-bookmarks".to_string(),
                source_label: "Firefox bookmarks".to_string(),
            }],
            is_pinned: true,
            history_entries: vec!["https://example.com".to_string()],
            history_index: 0,
            thumbnail_png: Some(vec![1, 2, 3]),
            thumbnail_width: 64,
            thumbnail_height: 48,
            favicon_rgba: Some(vec![255, 0, 0, 255]),
            favicon_width: 1,
            favicon_height: 1,
            session_state: Some(PersistedNodeSessionState {
                history_entries: vec![
                    "https://example.com".to_string(),
                    "https://example.com/docs".to_string(),
                ],
                history_index: 1,
                scroll_x: Some(12.0),
                scroll_y: Some(345.0),
                form_draft: Some("draft body".to_string()),
            }),
            mime_hint: Some("text/html".to_string()),
            classifications: Vec::new(),
            frame_layout_hints: Vec::new(),
            frame_split_offer_suppressed: false,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&node).unwrap();
        let archived = rkyv::access::<ArchivedPersistedNode, rkyv::rancor::Error>(&bytes).unwrap();
        assert!(!archived.node_id.as_str().is_empty());
        assert_eq!(archived.address.as_url_str(), "https://example.com");
        assert_eq!(archived.title.as_str(), "Example");
        assert_eq!(archived.position_x, 100.0);
        assert_eq!(archived.position_y, 200.0);
        assert_eq!(archived.tags.len(), 2);
        assert_eq!(archived.import_provenance.len(), 1);
        assert!(archived.is_pinned);
        assert_eq!(archived.history_entries.len(), 1);
        assert_eq!(archived.history_index, 0);
        assert_eq!(archived.thumbnail_png.as_ref().unwrap().len(), 3);
        assert_eq!(archived.thumbnail_width, 64);
        assert_eq!(archived.thumbnail_height, 48);
        assert_eq!(archived.favicon_rgba.as_ref().unwrap().len(), 4);
        assert_eq!(archived.favicon_width, 1);
        assert_eq!(archived.favicon_height, 1);
        let session = archived.session_state.as_ref().unwrap();
        assert_eq!(session.history_entries.len(), 2);
        assert_eq!(session.history_index, 1);
        assert_eq!(session.scroll_x, Some(12.0));
        assert_eq!(session.scroll_y, Some(345.0));
        assert_eq!(session.form_draft.as_ref().unwrap().as_str(), "draft body");
        assert_eq!(archived.mime_hint.as_ref().unwrap().as_str(), "text/html");
        assert_eq!(archived.address.as_url_str(), "https://example.com");
    }

    #[test]
    fn test_persisted_edge_roundtrip() {
        let edge = PersistedEdge {
            from_node_id: Uuid::new_v4().to_string(),
            to_node_id: Uuid::new_v4().to_string(),
            families: vec![PersistedEdgeFamily::Semantic],
            semantic: Some(PersistedSemanticEdgeData {
                sub_kinds: vec![PersistedSemanticSubKind::Hyperlink],
                label: None,
                agent_decay_progress: None,
            }),
            traversal: None,
            containment: None,
            arrangement: None,
            imported: None,
            provenance: None,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&edge).unwrap();
        let archived = rkyv::access::<ArchivedPersistedEdge, rkyv::rancor::Error>(&bytes).unwrap();
        assert!(!archived.from_node_id.as_str().is_empty());
        assert!(!archived.to_node_id.as_str().is_empty());
        assert_eq!(archived.families.len(), 1);
        assert!(archived.semantic.is_some());
    }

    #[test]
    fn test_persisted_edge_roundtrip_user_grouped() {
        let edge = PersistedEdge {
            from_node_id: Uuid::new_v4().to_string(),
            to_node_id: Uuid::new_v4().to_string(),
            families: vec![PersistedEdgeFamily::Semantic],
            semantic: Some(PersistedSemanticEdgeData {
                sub_kinds: vec![PersistedSemanticSubKind::UserGrouped],
                label: Some("tab-group".to_string()),
                agent_decay_progress: None,
            }),
            traversal: None,
            containment: None,
            arrangement: None,
            imported: None,
            provenance: None,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&edge).unwrap();
        let archived = rkyv::access::<ArchivedPersistedEdge, rkyv::rancor::Error>(&bytes).unwrap();
        assert!(archived.semantic.is_some());
    }

    #[test]
    fn test_graph_snapshot_roundtrip() {
        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: Uuid::new_v4().to_string(),
                address: PersistedAddress::Http("https://a.com".to_string()),
                url: "https://a.com".to_string(),
                cached_host: Some("a.com".to_string()),
                title: "A".to_string(),
                position_x: 0.0,
                position_y: 0.0,
                tags: vec![],
                tag_presentation: NodeTagPresentationState::default(),
                import_provenance: vec![],
                is_pinned: false,
                history_entries: vec![],
                history_index: 0,
                thumbnail_png: None,
                thumbnail_width: 0,
                thumbnail_height: 0,
                favicon_rgba: None,
                favicon_width: 0,
                favicon_height: 0,
                session_state: None,
                mime_hint: None,
                classifications: Vec::new(),
                frame_layout_hints: Vec::new(),
                frame_split_offer_suppressed: false,
            }],
            edges: vec![],
            import_records: vec![ImportRecord {
                record_id: "import-record:firefox-bookmarks".to_string(),
                source_id: "import:firefox-bookmarks".to_string(),
                source_label: "Firefox bookmarks".to_string(),
                imported_at_secs: 1234567000,
                memberships: vec![crate::graph::ImportRecordMembership {
                    node_id: Uuid::new_v4().to_string(),
                    suppressed: false,
                }],
            }],
            timestamp_secs: 1234567890,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot).unwrap();
        let archived = rkyv::access::<ArchivedGraphSnapshot, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(archived.nodes.len(), 1);
        assert_eq!(archived.edges.len(), 0);
        assert_eq!(archived.import_records.len(), 1);
        assert_eq!(archived.timestamp_secs, 1234567890);
    }

    #[test]
    fn test_log_entry_add_node_roundtrip() {
        let entry = LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "https://example.com".to_string(),
            position_x: 50.0,
            position_y: 75.0,
            timestamp_ms: 0,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::AddNode {
                node_id,
                url,
                position_x,
                position_y,
                ..
            } => {
                assert!(!node_id.as_str().is_empty());
                assert_eq!(url.as_str(), "https://example.com");
                assert_eq!(*position_x, 50.0);
                assert_eq!(*position_y, 75.0);
            }
            _ => panic!("Expected AddNode variant"),
        }
    }

    #[test]
    fn test_log_entry_update_node_url_roundtrip() {
        let entry = LogEntry::UpdateNodeUrl {
            node_id: Uuid::new_v4().to_string(),
            new_url: "https://new.com".to_string(),
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::UpdateNodeUrl { node_id, new_url } => {
                assert!(!node_id.as_str().is_empty());
                assert_eq!(new_url.as_str(), "https://new.com");
            }
            _ => panic!("Expected UpdateNodeUrl variant"),
        }
    }

    #[test]
    fn test_log_entry_remove_edge_roundtrip() {
        let entry = LogEntry::RemoveEdge {
            from_node_id: Uuid::new_v4().to_string(),
            to_node_id: Uuid::new_v4().to_string(),
            selector: PersistedRelationSelector::Semantic(PersistedSemanticSubKind::UserGrouped),
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::RemoveEdge {
                from_node_id,
                to_node_id,
                selector,
            } => {
                assert!(!from_node_id.as_str().is_empty());
                assert!(!to_node_id.as_str().is_empty());
                assert_eq!(
                    *selector,
                    ArchivedPersistedRelationSelector::Semantic(
                        ArchivedPersistedSemanticSubKind::UserGrouped,
                    ),
                );
            }
            _ => panic!("Expected RemoveEdge variant"),
        }
    }

    #[test]
    fn test_log_entry_append_traversal_roundtrip() {
        let entry = LogEntry::AppendTraversal {
            from_node_id: Uuid::new_v4().to_string(),
            to_node_id: Uuid::new_v4().to_string(),
            timestamp_ms: 1234,
            trigger: PersistedNavigationTrigger::Back,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::AppendTraversal {
                from_node_id,
                to_node_id,
                timestamp_ms,
                trigger,
            } => {
                assert!(!from_node_id.as_str().is_empty());
                assert!(!to_node_id.as_str().is_empty());
                assert_eq!(*timestamp_ms, 1234);
                assert_eq!(*trigger, ArchivedPersistedNavigationTrigger::Back);
            }
            _ => panic!("Expected AppendTraversal variant"),
        }
    }

    #[test]
    fn test_log_entry_update_node_mime_hint_roundtrip() {
        let node_id = Uuid::new_v4().to_string();

        // Set hint
        let entry = LogEntry::UpdateNodeMimeHint {
            node_id: node_id.clone(),
            mime_hint: Some("application/pdf".to_string()),
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::UpdateNodeMimeHint {
                node_id: id,
                mime_hint,
            } => {
                assert_eq!(id.as_str(), node_id);
                assert_eq!(mime_hint.as_ref().unwrap().as_str(), "application/pdf");
            }
            _ => panic!("Expected UpdateNodeMimeHint variant"),
        }

        // Clear hint
        let entry_clear = LogEntry::UpdateNodeMimeHint {
            node_id: node_id.clone(),
            mime_hint: None,
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry_clear).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::UpdateNodeMimeHint { mime_hint, .. } => {
                assert!(mime_hint.is_none());
            }
            _ => panic!("Expected UpdateNodeMimeHint variant"),
        }
    }

    #[test]
    fn test_record_frame_layout_hint_roundtrip() {
        let entry = LogEntry::RecordFrameLayoutHint {
            frame_id: Uuid::new_v4().to_string(),
            hint: FrameLayoutHint::SplitHalf {
                first: Uuid::new_v4().to_string(),
                second: Uuid::new_v4().to_string(),
                orientation: crate::graph::SplitOrientation::Vertical,
            },
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::RecordFrameLayoutHint { frame_id, .. } => {
                assert!(!frame_id.as_str().is_empty());
            }
            _ => panic!("Expected RecordFrameLayoutHint variant"),
        }
    }

    #[test]
    fn test_persisted_address_kind_roundtrip() {
        for kind in [
            PersistedAddressKind::Http,
            PersistedAddressKind::File,
            PersistedAddressKind::Data,
            PersistedAddressKind::GraphshellClip,
            PersistedAddressKind::Directory,
            PersistedAddressKind::Unknown,
        ] {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&kind).unwrap();
            let archived =
                rkyv::access::<ArchivedPersistedAddressKind, rkyv::rancor::Error>(&bytes).unwrap();
            let expected = match kind {
                PersistedAddressKind::Http => ArchivedPersistedAddressKind::Http,
                PersistedAddressKind::File => ArchivedPersistedAddressKind::File,
                PersistedAddressKind::Data => ArchivedPersistedAddressKind::Data,
                PersistedAddressKind::GraphshellClip => {
                    ArchivedPersistedAddressKind::GraphshellClip
                }
                PersistedAddressKind::Directory => ArchivedPersistedAddressKind::Directory,
                PersistedAddressKind::Unknown => ArchivedPersistedAddressKind::Unknown,
            };
            assert_eq!(*archived, expected);
        }
    }

    #[test]
    fn legacy_persisted_node_without_address_field_deserializes_via_url_fallback() {
        // Simulate a JSON snapshot written before Stage C.2 — it has `url` but no `address` field.
        // The `#[serde(default)]` on `address` produces `PersistedAddress::Custom("")`.
        // The `from_snapshot` load path detects the empty address URL and falls back to the `url` field.
        let json = r#"{
            "node_id": "00000000-0000-0000-0000-000000000001",
            "url": "https://legacy.example.com",
            "title": "Legacy Node",
            "position_x": 0.0,
            "position_y": 0.0,
            "tags": [],
            "is_pinned": false,
            "history_entries": [],
            "history_index": 0,
            "thumbnail_width": 0,
            "thumbnail_height": 0,
            "favicon_width": 0,
            "favicon_height": 0,
            "address_kind": "Http"
        }"#;
        let node: PersistedNode = serde_json::from_str(json).unwrap();
        // address field defaults to Custom("") — load path uses url field
        assert_eq!(node.url, "https://legacy.example.com");
        assert_eq!(node.address.as_url_str(), ""); // default fallback sentinel
    }

    #[test]
    fn new_persisted_node_with_address_field_uses_address_not_url() {
        let node = PersistedNode {
            node_id: "00000000-0000-0000-0000-000000000002".to_string(),
            address: PersistedAddress::File("file:///home/user/doc.txt".to_string()),
            url: "file:///home/user/doc.txt".to_string(),
            cached_host: None,
            title: "Doc".to_string(),
            position_x: 0.0,
            position_y: 0.0,
            tags: vec![],
            tag_presentation: NodeTagPresentationState::default(),
            import_provenance: vec![],
            is_pinned: false,
            history_entries: vec![],
            history_index: 0,
            thumbnail_png: None,
            thumbnail_width: 0,
            thumbnail_height: 0,
            favicon_rgba: None,
            favicon_width: 0,
            favicon_height: 0,
            session_state: None,
            mime_hint: None,
            classifications: vec![],
            frame_layout_hints: vec![],
            frame_split_offer_suppressed: false,
        };
        assert_eq!(node.address.as_url_str(), "file:///home/user/doc.txt");
        let json = serde_json::to_string(&node).unwrap();
        let restored: PersistedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.address.as_url_str(), "file:///home/user/doc.txt");
        assert!(matches!(restored.address, PersistedAddress::File(_)));
    }
}
