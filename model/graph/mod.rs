/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph data structures for the spatial browser.
//!
//! Core structures:
//! - `Graph`: Main graph container backed by petgraph::StableGraph
//! - `Node`: Webpage node with position, velocity, and metadata
//! - `EdgePayload`: Edge semantics and traversal events between nodes
//!
//! Boundary: direct mutation methods are `pub(crate)` and reserved for
//! trusted writers only.
//!
//! Trusted writers:
//! - reducer-owned mutation flow in `GraphBrowserApp`
//! - persistence replay/recovery code that reconstructs previously accepted
//!   reducer state
//!
//! This is an internal trust boundary, not reducer-only compiler enforcement.

use euclid::default::{Point2D, Vector2D};
use petgraph::algo::{astar, dijkstra, has_path_connecting, kosaraju_scc};
use petgraph::stable_graph::{EdgeIndex, NodeIndex, StableGraph};
use petgraph::visit::UndirectedAdaptor;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use petgraph::{Directed, Direction};
use rkyv::{
    Archive, Archived, Deserialize, Place, Resolver, Serialize,
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::services::persistence::types::{
    GraphSnapshot, PersistedAddressKind, PersistedArrangementEdgeData, PersistedArrangementSubKind,
    PersistedContainmentEdgeData, PersistedContainmentSubKind, PersistedEdge, PersistedEdgeFamily,
    PersistedImportedEdgeData, PersistedImportedSubKind, PersistedNavigationTrigger, PersistedNode,
    PersistedNodeSessionState, PersistedProvenanceEdgeData, PersistedProvenanceSubKind,
    PersistedSemanticEdgeData, PersistedSemanticSubKind, PersistedTraversalEdgeData,
    PersistedTraversalMetrics, PersistedTraversalRecord,
};

pub mod apply;
pub mod badge;
pub mod edge_style_registry;
pub mod egui_adapter;
pub mod facet_projection;
pub mod filter;

use self::badge::NodeTagPresentationState;

/// Stable node handle (petgraph NodeIndex — survives other deletions)
pub type NodeKey = NodeIndex;

/// Durable member reference used by frame layout hints.
///
/// `NodeKey` is process-local and not stable across restart, so persistent frame
/// layout metadata uses the member node's stable UUID string instead.
pub type FrameLayoutNodeId = String;

/// Graph backend direction type exposed for adapter integration.
pub(crate) type GraphDirection = Directed;

/// Graph backend index type exposed for adapter integration.
pub(crate) type GraphIndex = petgraph::graph::DefaultIx;

struct UuidAsBytes;

impl ArchiveWith<Uuid> for UuidAsBytes {
    type Archived = Archived<[u8; 16]>;
    type Resolver = Resolver<[u8; 16]>;

    fn resolve_with(field: &Uuid, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let bytes = *field.as_bytes();
        bytes.resolve(resolver, out);
    }
}

impl<S> SerializeWith<Uuid, S> for UuidAsBytes
where
    S: Fallible + ?Sized,
    [u8; 16]: Serialize<S>,
{
    fn serialize_with(field: &Uuid, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        let bytes = *field.as_bytes();
        bytes.serialize(serializer)
    }
}

impl<D> DeserializeWith<Archived<[u8; 16]>, Uuid, D> for UuidAsBytes
where
    D: Fallible + ?Sized,
    Archived<[u8; 16]>: Deserialize<[u8; 16], D>,
{
    fn deserialize_with(
        field: &Archived<[u8; 16]>,
        deserializer: &mut D,
    ) -> Result<Uuid, D::Error> {
        let bytes = field.deserialize(deserializer)?;
        Ok(Uuid::from_bytes(bytes))
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq, Eq))]
pub enum SplitOrientation {
    Vertical,
    Horizontal,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq, Eq))]
pub enum DominantEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq, Eq))]
pub enum FrameLayoutHint {
    SplitHalf {
        first: FrameLayoutNodeId,
        second: FrameLayoutNodeId,
        orientation: SplitOrientation,
    },
    SplitPamphlet {
        members: [FrameLayoutNodeId; 3],
        orientation: SplitOrientation,
    },
    SplitTriptych {
        dominant: FrameLayoutNodeId,
        dominant_edge: DominantEdge,
        wings: [FrameLayoutNodeId; 2],
    },
    SplitQuartered {
        top_left: FrameLayoutNodeId,
        top_right: FrameLayoutNodeId,
        bottom_left: FrameLayoutNodeId,
        bottom_right: FrameLayoutNodeId,
    },
}

struct Point2DAsTuple;

impl ArchiveWith<Point2D<f32>> for Point2DAsTuple {
    type Archived = Archived<(f32, f32)>;
    type Resolver = Resolver<(f32, f32)>;

    fn resolve_with(field: &Point2D<f32>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let value = (field.x, field.y);
        value.resolve(resolver, out);
    }
}

impl<S> SerializeWith<Point2D<f32>, S> for Point2DAsTuple
where
    S: Fallible + ?Sized,
    (f32, f32): Serialize<S>,
{
    fn serialize_with(
        field: &Point2D<f32>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        let value = (field.x, field.y);
        value.serialize(serializer)
    }
}

impl<D> DeserializeWith<Archived<(f32, f32)>, Point2D<f32>, D> for Point2DAsTuple
where
    D: Fallible + ?Sized,
    Archived<(f32, f32)>: Deserialize<(f32, f32), D>,
{
    fn deserialize_with(
        field: &Archived<(f32, f32)>,
        deserializer: &mut D,
    ) -> Result<Point2D<f32>, D::Error> {
        let (x, y) = field.deserialize(deserializer)?;
        Ok(Point2D::new(x, y))
    }
}

struct Vector2DAsTuple;

impl ArchiveWith<Vector2D<f32>> for Vector2DAsTuple {
    type Archived = Archived<(f32, f32)>;
    type Resolver = Resolver<(f32, f32)>;

    fn resolve_with(field: &Vector2D<f32>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let value = (field.x, field.y);
        value.resolve(resolver, out);
    }
}

impl<S> SerializeWith<Vector2D<f32>, S> for Vector2DAsTuple
where
    S: Fallible + ?Sized,
    (f32, f32): Serialize<S>,
{
    fn serialize_with(
        field: &Vector2D<f32>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        let value = (field.x, field.y);
        value.serialize(serializer)
    }
}

impl<D> DeserializeWith<Archived<(f32, f32)>, Vector2D<f32>, D> for Vector2DAsTuple
where
    D: Fallible + ?Sized,
    Archived<(f32, f32)>: Deserialize<(f32, f32), D>,
{
    fn deserialize_with(
        field: &Archived<(f32, f32)>,
        deserializer: &mut D,
    ) -> Result<Vector2D<f32>, D::Error> {
        let (x, y) = field.deserialize(deserializer)?;
        Ok(Vector2D::new(x, y))
    }
}

/// Address type hint for renderer selection.
///
/// Set automatically from the URL scheme at node creation time; can be overridden
/// by WAL entry `UpdateNodeAddressKind` when a more precise classification is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Archive, Serialize, Deserialize)]
pub enum AddressKind {
    /// Served over HTTP/HTTPS — default; Servo renders.
    #[default]
    Http,
    /// Local filesystem path (`file://` URL).
    File,
    /// Inline data URL payload.
    Data,
    /// Graphshell clip-address route (`verso://clip/...` or legacy `graphshell://clip/...`).
    GraphshellClip,
    /// Local filesystem directory path.
    Directory,
    /// Any other or unresolved scheme.
    Unknown,
}

/// Infer `AddressKind` from a URL scheme.
pub(crate) fn address_kind_from_url(url: &str) -> AddressKind {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        AddressKind::Http
    } else if lower.starts_with("data:") {
        AddressKind::Data
    } else if lower.starts_with("verso://clip/") || lower.starts_with("graphshell://clip/") {
        AddressKind::GraphshellClip
    } else if lower.starts_with("file://") {
        if file_url_uses_directory_syntax(url) {
            AddressKind::Directory
        } else {
            AddressKind::File
        }
    } else {
        AddressKind::Unknown
    }
}

fn file_url_uses_directory_syntax(url: &str) -> bool {
    // AddressKind classification must be deterministic from URL semantics alone,
    // independent of local filesystem state.
    url::Url::parse(url)
        .ok()
        .is_some_and(|parsed| parsed.path().ends_with('/'))
}

pub(crate) fn cached_host_from_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
}

/// Detect MIME type from URL + optional content bytes.
///
/// Detection order:
/// 1) Extension lookup via `mime_guess` (cheap, synchronous)
/// 2) Content-byte sniffing via `infer` only when extension lookup is
///    missing or ambiguous
///
/// Returns `None` when neither source yields a known MIME type.
pub(crate) fn detect_mime(url: &str, content_bytes: Option<&[u8]>) -> Option<String> {
    let no_fragment = url.split('#').next().unwrap_or(url);
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    // Strip file:// scheme so mime_guess sees a plain path.
    let path = no_query
        .strip_prefix("file://")
        .unwrap_or(no_query)
        .trim_start_matches('/');
    // Reconstruct a rooted path string for mime_guess.
    let guess_path = format!("/{path}");
    let guessed: Vec<String> = mime_guess::from_path(&guess_path)
        .into_iter()
        .map(|m| m.to_string())
        .collect();

    let is_ambiguous = guessed.len() > 1
        || guessed
            .first()
            .map(|m| m == "application/octet-stream")
            .unwrap_or(false);

    if !guessed.is_empty() && !is_ambiguous {
        return guessed.first().cloned();
    }

    if let Some(bytes) = content_bytes {
        if let Some(kind) = infer::get(bytes) {
            return Some(kind.mime_type().to_string());
        }
    }

    guessed.first().cloned()
}

/// Stable edge handle (petgraph EdgeIndex)
pub type EdgeKey = EdgeIndex;

/// Traversal archive payload emitted when dissolving a node.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub(crate) struct DissolvedTraversalRecord {
    #[rkyv(with = UuidAsBytes)]
    pub(crate) from_node_id: Uuid,
    #[rkyv(with = UuidAsBytes)]
    pub(crate) to_node_id: Uuid,
    pub(crate) traversals: Vec<Traversal>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct NodeImportProvenance {
    pub source_id: String,
    pub source_label: String,
}

// ---------------------------------------------------------------------------
// Node classification — Stage A durable enrichment schema
// Spec: graph_enrichment_plan.md §§ Core Data Model, Stage A
// ---------------------------------------------------------------------------

/// Classification scheme identifier (spec §Core Data Model).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum ClassificationScheme {
    /// Universal Decimal Classification (primary semantic taxonomy).
    #[default]
    Udc,
    /// Content-kind classification (page, article, repo, …).
    ContentKind,
    /// Custom namespaced scheme (e.g. `"myns:custom"`).
    Custom(String),
}

/// Origin of a classification or tag.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum ClassificationProvenance {
    /// Explicitly authored by the user.
    #[default]
    UserAuthored,
    /// Imported from an external data source (bookmarks, history, file, …).
    Imported,
    /// Inherited from a source/parent node relationship.
    InheritedFromSource,
    /// Derived by the knowledge registry (UDC lookup, content analysis, …).
    RegistryDerived,
    /// Proposed by an agent/model; not yet accepted by the user.
    AgentSuggested,
    /// Synced from the community/Verse network.
    CommunitySynced,
}

/// Lifecycle status of a classification record.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum ClassificationStatus {
    /// User has explicitly accepted this classification.
    Accepted,
    /// Proposed but not yet reviewed (e.g. agent-suggested).
    #[default]
    Suggested,
    /// User has explicitly rejected this classification.
    Rejected,
    /// Verified by an authoritative external source.
    Verified,
    /// Imported from an external record without explicit user review.
    Imported,
}

/// A single provenance-bearing classification record on a node (spec §Core Data Model).
///
/// Multiple records can coexist; at most one should have `primary: true` per scheme.
#[derive(
    Debug, Clone, PartialEq, Archive, Serialize, Deserialize, serde::Serialize, serde::Deserialize,
)]
pub struct NodeClassification {
    pub scheme: ClassificationScheme,
    /// Scheme-specific classification value (e.g. `"udc:519.6"`, `"article"`).
    pub value: String,
    /// Human-readable label resolved from the scheme (e.g. `"Computational mathematics"`).
    pub label: Option<String>,
    /// Confidence score in `[0.0, 1.0]`; `1.0` for user-authored.
    pub confidence: f32,
    pub provenance: ClassificationProvenance,
    pub status: ClassificationStatus,
    /// Whether this is the primary presentation classification for its scheme.
    pub primary: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ImportRecordMembership {
    pub node_id: String,
    pub suppressed: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ImportRecord {
    pub record_id: String,
    pub source_id: String,
    pub source_label: String,
    pub imported_at_secs: u64,
    pub memberships: Vec<ImportRecordMembership>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeImportRecordSummary {
    pub record_id: String,
    pub source_id: String,
    pub source_label: String,
    pub imported_at_secs: u64,
}

pub(crate) fn format_imported_at_secs(imported_at_secs: u64) -> String {
    time::OffsetDateTime::from_unix_timestamp(imported_at_secs as i64)
        .ok()
        .and_then(|timestamp| {
            timestamp
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| format!("unix:{imported_at_secs}"))
}

fn normalize_import_record_memberships(memberships: &mut Vec<ImportRecordMembership>) {
    memberships.sort_by(|left, right| {
        left.node_id
            .cmp(&right.node_id)
            .then_with(|| left.suppressed.cmp(&right.suppressed))
    });
    let mut deduped: Vec<ImportRecordMembership> = Vec::with_capacity(memberships.len());
    for membership in memberships.drain(..) {
        if let Some(existing) = deduped.last_mut()
            && existing.node_id == membership.node_id
        {
            existing.suppressed &= membership.suppressed;
            continue;
        }
        deduped.push(membership);
    }
    *memberships = deduped;
}

fn normalize_import_records(import_records: &mut Vec<ImportRecord>) {
    let mut merged = BTreeMap::<String, ImportRecord>::new();
    for mut record in import_records.drain(..) {
        normalize_import_record_memberships(&mut record.memberships);
        if record.record_id.trim().is_empty() {
            continue;
        }
        let entry = merged
            .entry(record.record_id.clone())
            .or_insert_with(|| ImportRecord {
                record_id: record.record_id.clone(),
                source_id: record.source_id.clone(),
                source_label: record.source_label.clone(),
                imported_at_secs: record.imported_at_secs,
                memberships: Vec::new(),
            });
        if entry.source_id.is_empty() {
            entry.source_id = record.source_id.clone();
        }
        if entry.source_label.is_empty() {
            entry.source_label = record.source_label.clone();
        }
        if entry.imported_at_secs == 0 {
            entry.imported_at_secs = record.imported_at_secs;
        } else if record.imported_at_secs != 0 {
            entry.imported_at_secs = entry.imported_at_secs.min(record.imported_at_secs);
        }
        entry.memberships.extend(record.memberships);
    }
    *import_records = merged.into_values().collect();
    for record in import_records.iter_mut() {
        normalize_import_record_memberships(&mut record.memberships);
    }
    import_records.sort_by(|left, right| {
        left.source_label
            .cmp(&right.source_label)
            .then_with(|| left.imported_at_secs.cmp(&right.imported_at_secs))
            .then_with(|| left.record_id.cmp(&right.record_id))
    });
}

fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A webpage node in the graph
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct Node {
    /// Stable node identity.
    #[rkyv(with = UuidAsBytes)]
    pub id: Uuid,

    /// Full URL of the webpage
    pub url: String,

    /// Cached hostname derived from `url` for UI label rendering.
    pub cached_host: Option<String>,

    /// Page title (or URL if no title)
    pub title: String,

    /// Transient projected position in graph space.
    ///
    /// Render and physics code may move this continuously between reducer
    /// commits.
    #[rkyv(with = Point2DAsTuple)]
    position: Point2D<f32>,

    /// Durable committed position used for snapshots and reducer-authored moves.
    #[rkyv(with = Point2DAsTuple)]
    committed_position: Point2D<f32>,

    /// Velocity for physics simulation
    #[rkyv(with = Vector2DAsTuple)]
    pub velocity: Vector2D<f32>,

    /// Canonical durable semantic tags for this node.
    pub tags: HashSet<String>,

    /// Presentation-only metadata for ordering and icon overrides.
    pub tag_presentation: NodeTagPresentationState,

    /// Derived external import provenance for this node.
    pub import_provenance: Vec<NodeImportProvenance>,

    /// Durable provenance-bearing classification records for this node.
    ///
    /// Spec: `graph_enrichment_plan.md §Core Data Model` — carries scheme, value,
    /// label, confidence, provenance, and status for each classification.
    pub classifications: Vec<NodeClassification>,

    /// Whether this node's position is pinned (doesn't move with physics)
    pub is_pinned: bool,

    /// Timestamp of last visit
    #[rkyv(with = rkyv::with::AsUnixTime)]
    pub last_visited: std::time::SystemTime,

    /// Navigation history seen for this node's mapped webview.
    pub history_entries: Vec<String>,

    /// Current index in `history_entries`.
    pub history_index: usize,

    /// Optional thumbnail bytes (PNG), persisted in snapshots.
    pub thumbnail_png: Option<Vec<u8>>,

    /// Thumbnail width in pixels (valid when `thumbnail_png` is `Some`).
    pub thumbnail_width: u32,

    /// Thumbnail height in pixels (valid when `thumbnail_png` is `Some`).
    pub thumbnail_height: u32,

    /// Optional favicon pixel data (RGBA8), persisted in snapshots.
    pub favicon_rgba: Option<Vec<u8>>,

    /// Favicon width in pixels (valid when `favicon_rgba` is `Some`).
    pub favicon_width: u32,

    /// Favicon height in pixels (valid when `favicon_rgba` is `Some`).
    pub favicon_height: u32,

    /// Last known scroll offset for higher-fidelity cold restore.
    pub session_scroll: Option<(f32, f32)>,

    /// Optional best-effort form draft payload (feature-guarded by caller policy).
    pub session_form_draft: Option<String>,

    /// Optional declared or sniffed MIME type; drives renderer selection.
    ///
    /// Set at node creation time from URL extension sniffing; may be updated by
    /// WAL entry `UpdateNodeMimeHint` when content-byte detection or a
    /// Content-Type header provides a more precise value.
    pub mime_hint: Option<String>,

    /// Address type hint (complement to `url` field).
    ///
    /// Inferred from the URL scheme at node creation time. May be overridden by
    /// WAL entry `UpdateNodeAddressKind`.
    pub address_kind: AddressKind,

    /// Durable split arrangement annotations for frame-anchor nodes.
    pub frame_layout_hints: Vec<FrameLayoutHint>,

    /// Durable opt-out for split-offer affordances on frame-anchor nodes.
    pub frame_split_offer_suppressed: bool,

    /// Webview lifecycle state
    pub lifecycle: NodeLifecycle,
}

/// Lifecycle state for webview management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum NodeLifecycle {
    /// Active webview (visible, rendering)
    Active,

    /// Warm webview (kept alive in memory but not currently visible in a pane)
    Warm,

    /// Cold (metadata only, no process)
    Cold,

    /// Tombstoned node retained for history/identity continuity but not live rendering/runtime.
    Tombstone,
}

impl Node {
    pub fn projected_position(&self) -> Point2D<f32> {
        self.position
    }

    pub fn committed_position(&self) -> Point2D<f32> {
        self.committed_position
    }

    #[cfg(test)]
    pub(crate) fn test_stub(url: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            url: url.to_string(),
            cached_host: cached_host_from_url(url),
            title: url.to_string(),
            position: Point2D::new(0.0, 0.0),
            committed_position: Point2D::new(0.0, 0.0),
            velocity: Vector2D::new(0.0, 0.0),
            tags: HashSet::new(),
            tag_presentation: NodeTagPresentationState::default(),
            import_provenance: Vec::new(),
            classifications: Vec::new(),
            is_pinned: false,
            last_visited: std::time::SystemTime::now(),
            history_entries: Vec::new(),
            history_index: 0,
            thumbnail_png: None,
            thumbnail_width: 0,
            thumbnail_height: 0,
            favicon_rgba: None,
            favicon_width: 0,
            favicon_height: 0,
            session_scroll: None,
            session_form_draft: None,
            mime_hint: None,
            address_kind: AddressKind::Http,
            frame_layout_hints: Vec::new(),
            frame_split_offer_suppressed: false,
            lifecycle: NodeLifecycle::Cold,
        }
    }
}

/// Type of edge connection
#[derive(Debug, Clone, Copy, PartialEq, Archive, Serialize, Deserialize)]
pub enum EdgeType {
    /// Hyperlink from one page to another
    Hyperlink,

    /// Browser history traversal
    History,

    /// Explicit user grouping association
    UserGrouped,

    /// Workbench/layout arrangement relation.
    ArrangementRelation(ArrangementSubKind),

    /// URL-derived containment hierarchy relation.
    ContainmentRelation(ContainmentSubKind),

    /// Relation imported from an external system (bookmarks folder, RSS feed, etc.).
    /// Derived-readonly at import time; promoted to durable only by explicit user action.
    ImportedRelation,

    /// Agent-inferred relation; provisional until accepted or evicted by decay.
    /// `decay_progress` is in [0.0, 1.0] — 0.0 = freshly asserted, 1.0 = at eviction threshold.
    AgentDerived { decay_progress: f32 },
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum EdgeFamily {
    Semantic,
    Traversal,
    Containment,
    Arrangement,
    Imported,
    Provenance,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum SemanticSubKind {
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
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum ArrangementSubKind {
    FrameMember,
    TileGroup,
    SplitPair,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum ContainmentSubKind {
    UrlPath,
    Domain,
    FileSystem,
    UserFolder,
    ClipSource,
    NotebookSection,
    CollectionMember,
}

impl ContainmentSubKind {
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::UrlPath => "url-path",
            Self::Domain => "domain",
            Self::FileSystem => "filesystem",
            Self::UserFolder => "user-folder",
            Self::ClipSource => "clip-source",
            Self::NotebookSection => "notebook-section",
            Self::CollectionMember => "collection-member",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum ImportedSubKind {
    BookmarkFolder,
    HistoryImport,
    RssMembership,
    FileSystemImport,
    ArchiveMembership,
    SharedCollection,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum ProvenanceSubKind {
    ClippedFrom,
    ExcerptedFrom,
    SummarizedFrom,
    TranslatedFrom,
    RewrittenFrom,
    GeneratedFrom,
    ExtractedFrom,
    ImportedFromSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum RelationDurability {
    Durable,
    Session,
}

impl RelationDurability {
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::Durable => "durable",
            Self::Session => "session",
        }
    }
}

impl ArrangementSubKind {
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::FrameMember => "frame-member",
            Self::TileGroup => "tile-group",
            Self::SplitPair => "split-pair",
        }
    }

    pub fn durability(self) -> RelationDurability {
        match self {
            Self::FrameMember => RelationDurability::Durable,
            Self::TileGroup | Self::SplitPair => RelationDurability::Session,
        }
    }

    pub fn provenance(self) -> &'static str {
        match self {
            Self::FrameMember => "workbench.frame_snapshot",
            Self::TileGroup => "workbench.tile_grouping",
            Self::SplitPair => "workbench.split_pairing",
        }
    }
}

/// Canonical edge kind set entry — internal index tag inside [`EdgePayload`].
/// Callers outside this module should use [`EdgeType`] with [`EdgePayload::has_edge_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub(crate) enum EdgeKind {
    Hyperlink,
    TraversalDerived,
    UserGrouped,
    AgentDerived,
    ArrangementRelation,
    ContainmentRelation,
    ImportedRelation,
}

#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub enum EdgeAssertion {
    Semantic {
        sub_kind: SemanticSubKind,
        label: Option<String>,
        decay_progress: Option<f32>,
    },
    Containment {
        sub_kind: ContainmentSubKind,
    },
    Arrangement {
        sub_kind: ArrangementSubKind,
    },
    Imported {
        sub_kind: ImportedSubKind,
    },
    Provenance {
        sub_kind: ProvenanceSubKind,
    },
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum RelationSelector {
    Family(EdgeFamily),
    Semantic(SemanticSubKind),
    Containment(ContainmentSubKind),
    Arrangement(ArrangementSubKind),
    Imported(ImportedSubKind),
    Provenance(ProvenanceSubKind),
}

/// Trigger classification for a traversal event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum NavigationTrigger {
    Unknown,
    LinkClick,
    Back,
    Forward,
    AddressBarEntry,
    PanePromotion,
    Programmatic,
}

impl NavigationTrigger {
    fn contributes_to_forward_count(self) -> bool {
        !matches!(self, Self::Back)
    }
}

/// A temporal traversal event recorded on an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct Traversal {
    pub timestamp_ms: u64,
    pub trigger: NavigationTrigger,
}

impl Traversal {
    pub fn now(trigger: NavigationTrigger) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            timestamp_ms,
            trigger,
        }
    }
}

/// Durable traversal aggregates retained even when rolling-window records are evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct EdgeMetrics {
    pub total_navigations: u64,
    pub forward_navigations: u64,
    pub backward_navigations: u64,
    pub last_navigated_at: Option<u64>,
}

impl EdgeMetrics {
    fn new() -> Self {
        Self {
            total_navigations: 0,
            forward_navigations: 0,
            backward_navigations: 0,
            last_navigated_at: None,
        }
    }

    fn record(&mut self, traversal: Traversal) {
        self.total_navigations = self.total_navigations.saturating_add(1);
        if traversal.trigger.contributes_to_forward_count() {
            self.forward_navigations = self.forward_navigations.saturating_add(1);
        } else {
            self.backward_navigations = self.backward_navigations.saturating_add(1);
        }
        self.last_navigated_at = Some(traversal.timestamp_ms);
    }
}

impl Default for EdgeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, Default)]
pub struct UserGroupedData {
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, Default)]
pub struct TraversalData {
    pub traversals: Vec<Traversal>,
    pub metrics: EdgeMetrics,
}

impl TraversalData {
    fn push(&mut self, traversal: Traversal) {
        self.metrics.record(traversal);
        self.traversals.push(traversal);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, Default)]
pub struct ArrangementData {
    pub sub_kinds: BTreeSet<ArrangementSubKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, Default)]
pub struct ContainmentData {
    pub sub_kinds: BTreeSet<ContainmentSubKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, Default)]
pub struct ImportedData {
    pub sub_kinds: BTreeSet<ImportedSubKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize, Default)]
pub struct ProvenanceData {
    pub sub_kinds: BTreeSet<ProvenanceSubKind>,
}

impl ContainmentData {
    fn insert(&mut self, sub_kind: ContainmentSubKind) -> bool {
        self.sub_kinds.insert(sub_kind)
    }

    fn remove(&mut self, sub_kind: ContainmentSubKind) -> bool {
        self.sub_kinds.remove(&sub_kind)
    }

    fn contains(&self, sub_kind: ContainmentSubKind) -> bool {
        self.sub_kinds.contains(&sub_kind)
    }

    fn is_empty(&self) -> bool {
        self.sub_kinds.is_empty()
    }
}

impl ArrangementData {
    fn insert(&mut self, sub_kind: ArrangementSubKind) -> bool {
        self.sub_kinds.insert(sub_kind)
    }

    fn remove(&mut self, sub_kind: ArrangementSubKind) -> bool {
        self.sub_kinds.remove(&sub_kind)
    }

    fn contains(&self, sub_kind: ArrangementSubKind) -> bool {
        self.sub_kinds.contains(&sub_kind)
    }

    fn is_empty(&self) -> bool {
        self.sub_kinds.is_empty()
    }

    fn has_durable_relation(&self) -> bool {
        self.sub_kinds
            .iter()
            .copied()
            .any(|sub_kind| sub_kind.durability() == RelationDurability::Durable)
    }

    fn has_session_relation(&self) -> bool {
        self.sub_kinds
            .iter()
            .copied()
            .any(|sub_kind| sub_kind.durability() == RelationDurability::Session)
    }
}

/// Edge semantics payload: structural assertions + temporal traversal events.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct EdgePayload {
    pub(crate) families: BTreeSet<EdgeFamily>,
    pub(crate) kinds: BTreeSet<EdgeKind>,
    pub user_grouped: Option<UserGroupedData>,
    pub traversal: Option<TraversalData>,
    pub arrangement: Option<ArrangementData>,
    pub containment: Option<ContainmentData>,
    pub imported: Option<ImportedData>,
    pub provenance: Option<ProvenanceData>,
}

impl EdgePayload {
    pub fn new() -> Self {
        Self {
            families: BTreeSet::new(),
            kinds: BTreeSet::new(),
            user_grouped: None,
            traversal: None,
            arrangement: None,
            containment: None,
            imported: None,
            provenance: None,
        }
    }

    pub fn from_edge_type(edge_type: EdgeType, label: Option<String>) -> Self {
        let mut payload = Self::new();
        let _ = payload.add_edge_kind(edge_type, label);
        payload
    }

    fn sync_family_from_kind(&mut self, kind: EdgeKind) {
        match kind {
            EdgeKind::Hyperlink | EdgeKind::UserGrouped | EdgeKind::AgentDerived => {
                let _ = self.families.insert(EdgeFamily::Semantic);
            }
            EdgeKind::TraversalDerived => {
                let _ = self.families.insert(EdgeFamily::Traversal);
            }
            EdgeKind::ArrangementRelation => {
                let _ = self.families.insert(EdgeFamily::Arrangement);
            }
            EdgeKind::ContainmentRelation => {
                let _ = self.families.insert(EdgeFamily::Containment);
            }
            EdgeKind::ImportedRelation => {
                let _ = self.families.insert(EdgeFamily::Imported);
            }
        }
    }

    fn prune_family(&mut self, family: EdgeFamily) {
        let keep = match family {
            EdgeFamily::Semantic => {
                self.kinds.contains(&EdgeKind::Hyperlink)
                    || self.kinds.contains(&EdgeKind::UserGrouped)
                    || self.kinds.contains(&EdgeKind::AgentDerived)
            }
            EdgeFamily::Traversal => self.kinds.contains(&EdgeKind::TraversalDerived),
            EdgeFamily::Containment => self.kinds.contains(&EdgeKind::ContainmentRelation),
            EdgeFamily::Arrangement => self.kinds.contains(&EdgeKind::ArrangementRelation),
            EdgeFamily::Imported => self.kinds.contains(&EdgeKind::ImportedRelation),
            EdgeFamily::Provenance => self
                .provenance
                .as_ref()
                .is_some_and(|data| !data.sub_kinds.is_empty()),
        };
        if !keep {
            self.families.remove(&family);
        }
    }

    pub fn assert_relation(&mut self, assertion: EdgeAssertion) -> bool {
        match assertion {
            EdgeAssertion::Semantic {
                sub_kind: SemanticSubKind::Hyperlink,
                label,
                ..
            } => self.add_edge_kind(EdgeType::Hyperlink, label),
            EdgeAssertion::Semantic {
                sub_kind: SemanticSubKind::UserGrouped,
                label,
                ..
            } => self.add_edge_kind(EdgeType::UserGrouped, label),
            EdgeAssertion::Semantic {
                sub_kind: SemanticSubKind::AgentDerived,
                decay_progress,
                ..
            } => self.add_edge_kind(
                EdgeType::AgentDerived {
                    decay_progress: decay_progress.unwrap_or(0.0),
                },
                None,
            ),
            EdgeAssertion::Semantic { .. } => false,
            EdgeAssertion::Containment { sub_kind } => {
                self.add_edge_kind(EdgeType::ContainmentRelation(sub_kind), None)
            }
            EdgeAssertion::Arrangement { sub_kind } => {
                self.add_edge_kind(EdgeType::ArrangementRelation(sub_kind), None)
            }
            EdgeAssertion::Imported { sub_kind } => {
                let inserted = self.add_edge_kind(EdgeType::ImportedRelation, None);
                let data = self.imported.get_or_insert_with(ImportedData::default);
                inserted | data.sub_kinds.insert(sub_kind)
            }
            EdgeAssertion::Provenance { sub_kind } => {
                let _ = self.families.insert(EdgeFamily::Provenance);
                let data = self.provenance.get_or_insert_with(ProvenanceData::default);
                data.sub_kinds.insert(sub_kind)
            }
        }
    }

    pub fn add_edge_kind(&mut self, edge_type: EdgeType, label: Option<String>) -> bool {
        match edge_type {
            EdgeType::Hyperlink => {
                let inserted = self.kinds.insert(EdgeKind::Hyperlink);
                self.sync_family_from_kind(EdgeKind::Hyperlink);
                inserted
            }
            EdgeType::UserGrouped => {
                let inserted = self.kinds.insert(EdgeKind::UserGrouped);
                self.sync_family_from_kind(EdgeKind::UserGrouped);
                let data = self
                    .user_grouped
                    .get_or_insert_with(UserGroupedData::default);
                if let Some(label) = label
                    && data.label.as_ref() != Some(&label)
                {
                    data.label = Some(label);
                    return true;
                }
                inserted
            }
            EdgeType::History => {
                let inserted = self.kinds.insert(EdgeKind::TraversalDerived);
                self.sync_family_from_kind(EdgeKind::TraversalDerived);
                let had_data = self.traversal.is_some();
                let _ = self.traversal.get_or_insert_with(TraversalData::default);
                inserted || !had_data
            }
            EdgeType::ArrangementRelation(sub_kind) => {
                let inserted = self.kinds.insert(EdgeKind::ArrangementRelation);
                self.sync_family_from_kind(EdgeKind::ArrangementRelation);
                let data = self
                    .arrangement
                    .get_or_insert_with(ArrangementData::default);
                inserted | data.insert(sub_kind)
            }
            EdgeType::ContainmentRelation(sub_kind) => {
                let inserted = self.kinds.insert(EdgeKind::ContainmentRelation);
                self.sync_family_from_kind(EdgeKind::ContainmentRelation);
                let data = self
                    .containment
                    .get_or_insert_with(ContainmentData::default);
                inserted | data.insert(sub_kind)
            }
            EdgeType::ImportedRelation => {
                let inserted = self.kinds.insert(EdgeKind::ImportedRelation);
                self.sync_family_from_kind(EdgeKind::ImportedRelation);
                let _ = self.imported.get_or_insert_with(ImportedData::default);
                inserted
            }
            EdgeType::AgentDerived { .. } => {
                let inserted = self.kinds.insert(EdgeKind::AgentDerived);
                self.sync_family_from_kind(EdgeKind::AgentDerived);
                inserted
            }
        }
    }

    pub fn add_edge_type(&mut self, edge_type: EdgeType) {
        let _ = self.add_edge_kind(edge_type, None);
    }

    pub fn has_relation(&self, selector: RelationSelector) -> bool {
        match selector {
            RelationSelector::Family(family) => self.families.contains(&family),
            RelationSelector::Semantic(SemanticSubKind::Hyperlink) => {
                self.has_edge_type(EdgeType::Hyperlink)
            }
            RelationSelector::Semantic(SemanticSubKind::UserGrouped) => {
                self.has_edge_type(EdgeType::UserGrouped)
            }
            RelationSelector::Semantic(SemanticSubKind::AgentDerived) => {
                self.has_edge_type(EdgeType::AgentDerived {
                    decay_progress: 0.0,
                })
            }
            RelationSelector::Semantic(_) => false,
            RelationSelector::Containment(sub_kind) => {
                self.has_edge_type(EdgeType::ContainmentRelation(sub_kind))
            }
            RelationSelector::Arrangement(sub_kind) => {
                self.has_edge_type(EdgeType::ArrangementRelation(sub_kind))
            }
            RelationSelector::Imported(sub_kind) => {
                self.imported
                    .as_ref()
                    .is_some_and(|data| data.sub_kinds.contains(&sub_kind))
                    || (self
                        .imported
                        .as_ref()
                        .is_some_and(|data| data.sub_kinds.is_empty())
                        && self.has_edge_type(EdgeType::ImportedRelation))
            }
            RelationSelector::Provenance(sub_kind) => self
                .provenance
                .as_ref()
                .is_some_and(|data| data.sub_kinds.contains(&sub_kind)),
        }
    }

    pub fn has_edge_kind(&self, edge_type: EdgeType) -> bool {
        match edge_type {
            EdgeType::Hyperlink => self.kinds.contains(&EdgeKind::Hyperlink),
            EdgeType::UserGrouped => {
                self.kinds.contains(&EdgeKind::UserGrouped) && self.user_grouped.is_some()
            }
            EdgeType::History => {
                self.kinds.contains(&EdgeKind::TraversalDerived) && self.traversal.is_some()
            }
            EdgeType::ArrangementRelation(sub_kind) => {
                self.kinds.contains(&EdgeKind::ArrangementRelation)
                    && self
                        .arrangement
                        .as_ref()
                        .is_some_and(|data| data.contains(sub_kind))
            }
            EdgeType::ContainmentRelation(sub_kind) => {
                self.kinds.contains(&EdgeKind::ContainmentRelation)
                    && self
                        .containment
                        .as_ref()
                        .is_some_and(|data| data.contains(sub_kind))
            }
            EdgeType::ImportedRelation => self.kinds.contains(&EdgeKind::ImportedRelation),
            EdgeType::AgentDerived { .. } => self.kinds.contains(&EdgeKind::AgentDerived),
        }
    }

    pub fn has_edge_type(&self, edge_type: EdgeType) -> bool {
        self.has_edge_kind(edge_type)
    }

    pub(crate) fn has_kind(&self, kind: EdgeKind) -> bool {
        self.kinds.contains(&kind)
    }

    pub fn retract_relation(&mut self, selector: RelationSelector) -> bool {
        match selector {
            RelationSelector::Family(EdgeFamily::Traversal) => {
                self.remove_edge_type(EdgeType::History)
            }
            RelationSelector::Family(_) => false,
            RelationSelector::Semantic(SemanticSubKind::Hyperlink) => {
                self.remove_edge_type(EdgeType::Hyperlink)
            }
            RelationSelector::Semantic(SemanticSubKind::UserGrouped) => {
                self.remove_edge_type(EdgeType::UserGrouped)
            }
            RelationSelector::Semantic(SemanticSubKind::AgentDerived) => {
                self.remove_edge_type(EdgeType::AgentDerived {
                    decay_progress: 0.0,
                })
            }
            RelationSelector::Semantic(_) => false,
            RelationSelector::Containment(sub_kind) => {
                self.remove_edge_type(EdgeType::ContainmentRelation(sub_kind))
            }
            RelationSelector::Arrangement(sub_kind) => {
                self.remove_edge_type(EdgeType::ArrangementRelation(sub_kind))
            }
            RelationSelector::Imported(sub_kind) => {
                if let Some(data) = self.imported.as_mut()
                    && data.sub_kinds.remove(&sub_kind)
                {
                    if data.sub_kinds.is_empty() {
                        self.imported = None;
                        let _ = self.remove_edge_type(EdgeType::ImportedRelation);
                    }
                    true
                } else {
                    self.remove_edge_type(EdgeType::ImportedRelation)
                }
            }
            RelationSelector::Provenance(sub_kind) => {
                if let Some(data) = self.provenance.as_mut()
                    && data.sub_kinds.remove(&sub_kind)
                {
                    if data.sub_kinds.is_empty() {
                        self.provenance = None;
                        self.prune_family(EdgeFamily::Provenance);
                    }
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn remove_edge_kind(&mut self, edge_type: EdgeType) -> bool {
        match edge_type {
            EdgeType::Hyperlink if self.kinds.remove(&EdgeKind::Hyperlink) => true,
            EdgeType::UserGrouped if self.kinds.remove(&EdgeKind::UserGrouped) => {
                self.user_grouped = None;
                self.prune_family(EdgeFamily::Semantic);
                true
            }
            EdgeType::History if self.kinds.remove(&EdgeKind::TraversalDerived) => {
                self.traversal = None;
                self.prune_family(EdgeFamily::Traversal);
                true
            }
            EdgeType::ArrangementRelation(sub_kind)
                if self
                    .arrangement
                    .as_mut()
                    .is_some_and(|data| data.remove(sub_kind)) =>
            {
                if self
                    .arrangement
                    .as_ref()
                    .is_some_and(ArrangementData::is_empty)
                {
                    self.arrangement = None;
                    self.kinds.remove(&EdgeKind::ArrangementRelation);
                    self.prune_family(EdgeFamily::Arrangement);
                }
                true
            }
            EdgeType::ContainmentRelation(sub_kind)
                if self
                    .containment
                    .as_mut()
                    .is_some_and(|data| data.remove(sub_kind)) =>
            {
                if self
                    .containment
                    .as_ref()
                    .is_some_and(ContainmentData::is_empty)
                {
                    self.containment = None;
                    self.kinds.remove(&EdgeKind::ContainmentRelation);
                    self.prune_family(EdgeFamily::Containment);
                }
                true
            }
            EdgeType::ImportedRelation if self.kinds.remove(&EdgeKind::ImportedRelation) => {
                self.imported = None;
                self.prune_family(EdgeFamily::Imported);
                true
            }
            EdgeType::AgentDerived { .. } if self.kinds.remove(&EdgeKind::AgentDerived) => {
                self.prune_family(EdgeFamily::Semantic);
                true
            }
            _ => false,
        }
    }

    pub fn remove_edge_type(&mut self, edge_type: EdgeType) -> bool {
        self.remove_edge_kind(edge_type)
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn label(&self) -> Option<&str> {
        self.user_grouped
            .as_ref()
            .and_then(|data| data.label.as_deref())
    }

    pub fn traversal_data(&self) -> Option<&TraversalData> {
        self.traversal.as_ref()
    }

    pub fn arrangement_data(&self) -> Option<&ArrangementData> {
        self.arrangement.as_ref()
    }

    pub fn containment_data(&self) -> Option<&ContainmentData> {
        self.containment.as_ref()
    }

    pub fn imported_data(&self) -> Option<&ImportedData> {
        self.imported.as_ref()
    }

    pub fn provenance_data(&self) -> Option<&ProvenanceData> {
        self.provenance.as_ref()
    }

    pub fn has_arrangement_sub_kind(&self, sub_kind: ArrangementSubKind) -> bool {
        self.arrangement
            .as_ref()
            .is_some_and(|data| data.contains(sub_kind))
    }

    pub fn has_durable_arrangement_relation(&self) -> bool {
        self.arrangement
            .as_ref()
            .is_some_and(ArrangementData::has_durable_relation)
    }

    pub fn has_session_arrangement_relation(&self) -> bool {
        self.arrangement
            .as_ref()
            .is_some_and(ArrangementData::has_session_relation)
    }

    pub fn traversals(&self) -> &[Traversal] {
        self.traversal
            .as_ref()
            .map(|data| data.traversals.as_slice())
            .unwrap_or(&[])
    }

    pub fn metrics(&self) -> EdgeMetrics {
        self.traversal
            .as_ref()
            .map(|data| data.metrics)
            .unwrap_or_default()
    }

    pub fn push_traversal(&mut self, traversal: Traversal) {
        let _ = self.kinds.insert(EdgeKind::TraversalDerived);
        self.sync_family_from_kind(EdgeKind::TraversalDerived);
        self.traversal
            .get_or_insert_with(TraversalData::default)
            .push(traversal);
    }

    pub fn families(&self) -> &BTreeSet<EdgeFamily> {
        &self.families
    }
}

impl Default for EdgePayload {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only view of an edge (built from petgraph edge references)
#[derive(Debug, Clone, Copy)]
pub struct EdgeView {
    pub from: NodeKey,
    pub to: NodeKey,
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrangementEdgeView {
    pub from: NodeKey,
    pub to: NodeKey,
    pub sub_kind: ArrangementSubKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainmentEdgeView {
    pub from: NodeKey,
    pub to: NodeKey,
    pub sub_kind: ContainmentSubKind,
}

/// Main graph structure backed by petgraph::StableGraph
#[derive(Clone)]
pub struct Graph {
    /// The underlying petgraph stable graph
    pub(crate) inner: StableGraph<Node, EdgePayload, Directed>,

    /// URL to node mapping for lookup (supports duplicate URLs).
    url_to_nodes: HashMap<String, Vec<NodeKey>>,

    /// Stable UUID to node mapping.
    id_to_node: HashMap<Uuid, NodeKey>,

    /// Durable imported relation truth; node provenance is derived from this.
    import_records: Vec<ImportRecord>,
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            inner: StableGraph::new(),
            url_to_nodes: HashMap::new(),
            id_to_node: HashMap::new(),
            import_records: Vec::new(),
        }
    }

    // Single-write-path boundary (Phase 6.5): graph topology mutators are
    // crate-internal and intended for trusted writers (reducer + persistence
    // replay/recovery). Other runtime/shell code paths should route through
    // reducer intents rather than calling topology mutators directly.

    /// Add a new node to the graph
    pub(crate) fn add_node(&mut self, url: String, position: Point2D<f32>) -> NodeKey {
        self.add_node_with_id(Uuid::new_v4(), url, position)
    }

    /// Add a node with a pre-existing UUID.
    pub(crate) fn add_node_with_id(
        &mut self,
        id: Uuid,
        url: String,
        position: Point2D<f32>,
    ) -> NodeKey {
        let now = std::time::SystemTime::now();
        let key = self.inner.add_node(Node {
            id,
            title: url.clone(),
            url: url.clone(),
            cached_host: cached_host_from_url(&url),
            position,
            committed_position: position,
            velocity: Vector2D::zero(),
            tags: HashSet::new(),
            tag_presentation: NodeTagPresentationState::default(),
            import_provenance: Vec::new(),
            classifications: Vec::new(),
            is_pinned: false,
            last_visited: now,
            history_entries: Vec::new(),
            history_index: 0,
            thumbnail_png: None,
            thumbnail_width: 0,
            thumbnail_height: 0,
            favicon_rgba: None,
            favicon_width: 0,
            favicon_height: 0,
            session_scroll: None,
            session_form_draft: None,
            mime_hint: detect_mime(&url, None),
            address_kind: address_kind_from_url(&url),
            frame_layout_hints: Vec::new(),
            frame_split_offer_suppressed: false,
            lifecycle: NodeLifecycle::Cold,
        });

        self.url_to_nodes.entry(url).or_default().push(key);
        self.id_to_node.insert(id, key);
        key
    }

    /// Remove a node and all its connected edges
    pub(crate) fn remove_node(&mut self, key: NodeKey) -> bool {
        if let Some(node) = self.inner.remove_node(key) {
            self.id_to_node.remove(&node.id);
            self.remove_url_mapping(&node.url, key);
            let removed_id = node.id.to_string();
            for record in &mut self.import_records {
                record
                    .memberships
                    .retain(|membership| membership.node_id != removed_id);
            }
            self.import_records
                .retain(|record| !record.memberships.is_empty());
            true
        } else {
            false
        }
    }

    /// Update a node's URL, maintaining the url_to_node index.
    /// Returns the old URL, or None if the node doesn't exist.
    pub(crate) fn update_node_url(&mut self, key: NodeKey, new_url: String) -> Option<String> {
        let node = self.inner.node_weight_mut(key)?;
        node.cached_host = cached_host_from_url(&new_url);
        let old_url = std::mem::replace(&mut node.url, new_url.clone());
        self.remove_url_mapping(&old_url, key);
        self.url_to_nodes.entry(new_url).or_default().push(key);
        Some(old_url)
    }

    pub fn recompute_cached_hosts(&mut self) {
        for node in self.inner.node_weights_mut() {
            node.cached_host = cached_host_from_url(&node.url);
        }
    }

    pub(crate) fn set_node_title(&mut self, key: NodeKey, title: String) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.title == title {
            return false;
        }
        node.title = title;
        true
    }

    pub(crate) fn set_node_thumbnail(
        &mut self,
        key: NodeKey,
        png_bytes: Vec<u8>,
        width: u32,
        height: u32,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.thumbnail_png.as_ref() == Some(&png_bytes)
            && node.thumbnail_width == width
            && node.thumbnail_height == height
        {
            return false;
        }
        node.thumbnail_png = Some(png_bytes);
        node.thumbnail_width = width;
        node.thumbnail_height = height;
        true
    }

    pub(crate) fn set_node_favicon(
        &mut self,
        key: NodeKey,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.favicon_rgba.as_ref() == Some(&rgba)
            && node.favicon_width == width
            && node.favicon_height == height
        {
            return false;
        }
        node.favicon_rgba = Some(rgba);
        node.favicon_width = width;
        node.favicon_height = height;
        true
    }

    pub(crate) fn set_node_mime_hint(&mut self, key: NodeKey, mime_hint: Option<String>) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.mime_hint == mime_hint {
            return false;
        }
        node.mime_hint = mime_hint;
        true
    }

    pub(crate) fn set_node_address_kind(&mut self, key: NodeKey, kind: AddressKind) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.address_kind == kind {
            return false;
        }
        node.address_kind = kind;
        true
    }

    pub(crate) fn set_node_pinned(&mut self, key: NodeKey, is_pinned: bool) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.is_pinned == is_pinned {
            return false;
        }
        node.is_pinned = is_pinned;
        true
    }

    pub(crate) fn append_frame_layout_hint(&mut self, key: NodeKey, hint: FrameLayoutHint) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        node.frame_layout_hints.push(hint);
        true
    }

    pub(crate) fn remove_frame_layout_hint_at(&mut self, key: NodeKey, hint_index: usize) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if hint_index >= node.frame_layout_hints.len() {
            return false;
        }
        node.frame_layout_hints.remove(hint_index);
        true
    }

    pub(crate) fn move_frame_layout_hint(
        &mut self,
        key: NodeKey,
        from_index: usize,
        to_index: usize,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if from_index >= node.frame_layout_hints.len()
            || to_index >= node.frame_layout_hints.len()
            || from_index == to_index
        {
            return false;
        }
        let hint = node.frame_layout_hints.remove(from_index);
        node.frame_layout_hints.insert(to_index, hint);
        true
    }

    pub(crate) fn set_frame_split_offer_suppressed(
        &mut self,
        key: NodeKey,
        suppressed: bool,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.frame_split_offer_suppressed == suppressed {
            return false;
        }
        node.frame_split_offer_suppressed = suppressed;
        true
    }

    pub(crate) fn frame_layout_hints(&self, key: NodeKey) -> Option<&[FrameLayoutHint]> {
        self.get_node(key)
            .map(|node| node.frame_layout_hints.as_slice())
    }

    pub(crate) fn frame_split_offer_suppressed(&self, key: NodeKey) -> Option<bool> {
        self.get_node(key)
            .map(|node| node.frame_split_offer_suppressed)
    }

    pub(crate) fn insert_node_tag(&mut self, key: NodeKey, tag: String) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let inserted = node.tags.insert(tag.clone());
        if inserted && !node.tag_presentation.ordered_tags.contains(&tag) {
            node.tag_presentation.ordered_tags.push(tag);
        }
        inserted
    }

    pub(crate) fn remove_node_tag(&mut self, key: NodeKey, tag: &str) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let removed = node.tags.remove(tag);
        if removed {
            node.tag_presentation
                .ordered_tags
                .retain(|entry| entry != tag);
            node.tag_presentation.icon_overrides.remove(tag);
        }
        removed
    }

    pub(crate) fn node_tags(&self, key: NodeKey) -> Option<&HashSet<String>> {
        self.get_node(key).map(|node| &node.tags)
    }

    pub(crate) fn node_tag_presentation(&self, key: NodeKey) -> Option<&NodeTagPresentationState> {
        self.get_node(key).map(|node| &node.tag_presentation)
    }

    pub(crate) fn node_import_provenance(&self, key: NodeKey) -> Option<&[NodeImportProvenance]> {
        self.get_node(key)
            .map(|node| node.import_provenance.as_slice())
    }

    // --- Classification accessors (Stage A) ---

    pub(crate) fn node_classifications(&self, key: NodeKey) -> Option<&[NodeClassification]> {
        self.get_node(key)
            .map(|node| node.classifications.as_slice())
    }

    /// Add a classification record to a node.
    ///
    /// Deduplicates by `(scheme, value)`. Returns `true` if the record was inserted.
    pub(crate) fn add_node_classification(
        &mut self,
        key: NodeKey,
        classification: NodeClassification,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let already_exists = node
            .classifications
            .iter()
            .any(|c| c.scheme == classification.scheme && c.value == classification.value);
        if already_exists {
            return false;
        }
        node.classifications.push(classification);
        true
    }

    /// Remove all classification records matching `(scheme, value)`.
    ///
    /// Returns `true` if at least one record was removed.
    pub(crate) fn remove_node_classification(
        &mut self,
        key: NodeKey,
        scheme: &ClassificationScheme,
        value: &str,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let before = node.classifications.len();
        node.classifications
            .retain(|c| !(c.scheme == *scheme && c.value == value));
        node.classifications.len() < before
    }

    /// Update the `status` of a classification record identified by `(scheme, value)`.
    ///
    /// Returns `true` if a matching record was found and updated.
    pub(crate) fn set_node_classification_status(
        &mut self,
        key: NodeKey,
        scheme: &ClassificationScheme,
        value: &str,
        status: ClassificationStatus,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let mut found = false;
        for c in node.classifications.iter_mut() {
            if c.scheme == *scheme && c.value == value {
                c.status = status.clone();
                found = true;
            }
        }
        found
    }

    /// Promote a classification record to primary for its scheme; demotes all others.
    ///
    /// Returns `true` if a matching record was found.
    pub(crate) fn set_node_primary_classification(
        &mut self,
        key: NodeKey,
        scheme: &ClassificationScheme,
        value: &str,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let mut found = false;
        for c in node.classifications.iter_mut() {
            if c.scheme == *scheme {
                c.primary = c.value == value;
                if c.value == value {
                    found = true;
                }
            }
        }
        found
    }

    pub(crate) fn import_records(&self) -> &[ImportRecord] {
        &self.import_records
    }

    pub(crate) fn import_record_summaries_for_node(
        &self,
        key: NodeKey,
    ) -> Vec<NodeImportRecordSummary> {
        let Some(node) = self.get_node(key) else {
            return Vec::new();
        };
        let node_id = node.id.to_string();
        let mut summaries = self
            .import_records
            .iter()
            .filter(|record| {
                record
                    .memberships
                    .iter()
                    .any(|membership| membership.node_id == node_id && !membership.suppressed)
            })
            .map(|record| NodeImportRecordSummary {
                record_id: record.record_id.clone(),
                source_id: record.source_id.clone(),
                source_label: record.source_label.clone(),
                imported_at_secs: record.imported_at_secs,
            })
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            right
                .imported_at_secs
                .cmp(&left.imported_at_secs)
                .then_with(|| left.source_label.cmp(&right.source_label))
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        summaries
    }

    pub(crate) fn import_record_member_keys(&self, record_id: &str) -> Vec<NodeKey> {
        let mut member_keys = self
            .import_records
            .iter()
            .find(|record| record.record_id == record_id)
            .into_iter()
            .flat_map(|record| record.memberships.iter())
            .filter(|membership| !membership.suppressed)
            .filter_map(|membership| Uuid::parse_str(&membership.node_id).ok())
            .filter_map(|node_id| self.id_to_node.get(&node_id).copied())
            .collect::<Vec<_>>();
        member_keys.sort_by_key(|key| key.index());
        member_keys.dedup();
        member_keys
    }

    pub(crate) fn delete_import_record(&mut self, record_id: &str) -> bool {
        let original_len = self.import_records.len();
        self.import_records
            .retain(|record| record.record_id != record_id);
        if self.import_records.len() == original_len {
            return false;
        }
        self.sync_node_import_provenance_from_records();
        true
    }

    pub(crate) fn set_import_record_membership_suppressed(
        &mut self,
        record_id: &str,
        key: NodeKey,
        suppressed: bool,
    ) -> bool {
        let Some(node_id) = self.get_node(key).map(|node| node.id.to_string()) else {
            return false;
        };
        let mut changed = false;
        for record in &mut self.import_records {
            if record.record_id != record_id {
                continue;
            }
            for membership in &mut record.memberships {
                if membership.node_id == node_id {
                    if membership.suppressed != suppressed {
                        membership.suppressed = suppressed;
                        changed = true;
                    }
                    break;
                }
            }
        }
        if changed {
            self.sync_node_import_provenance_from_records();
        }
        changed
    }

    pub(crate) fn set_import_records(&mut self, mut import_records: Vec<ImportRecord>) -> bool {
        normalize_import_records(&mut import_records);
        if self.import_records == import_records {
            return false;
        }
        self.import_records = import_records;
        self.sync_node_import_provenance_from_records();
        true
    }

    fn sync_node_import_provenance_from_records(&mut self) {
        let mut provenance_by_node = HashMap::<NodeKey, Vec<NodeImportProvenance>>::new();
        for record in &self.import_records {
            for membership in record
                .memberships
                .iter()
                .filter(|membership| !membership.suppressed)
            {
                let Ok(node_id) = Uuid::parse_str(&membership.node_id) else {
                    continue;
                };
                let Some(&node_key) = self.id_to_node.get(&node_id) else {
                    continue;
                };
                provenance_by_node
                    .entry(node_key)
                    .or_default()
                    .push(NodeImportProvenance {
                        source_id: record.source_id.clone(),
                        source_label: record.source_label.clone(),
                    });
            }
        }

        let node_keys = self.inner.node_indices().collect::<Vec<_>>();
        for node_key in node_keys {
            let Some(node) = self.inner.node_weight_mut(node_key) else {
                continue;
            };
            let mut provenance = provenance_by_node.remove(&node_key).unwrap_or_default();
            provenance.sort();
            provenance.dedup();
            node.import_provenance = provenance;
        }
    }

    fn rebuild_import_records_from_node_provenance(&mut self, imported_at_secs: u64) {
        let existing_record_meta = self
            .import_records
            .iter()
            .map(|record| {
                (
                    (record.source_id.clone(), record.source_label.clone()),
                    (record.record_id.clone(), record.imported_at_secs),
                )
            })
            .collect::<HashMap<_, _>>();

        let mut grouped = BTreeMap::<(String, String), Vec<ImportRecordMembership>>::new();
        for (_node_key, node) in self.nodes() {
            let node_id = node.id.to_string();
            for provenance in &node.import_provenance {
                grouped
                    .entry((
                        provenance.source_id.clone(),
                        provenance.source_label.clone(),
                    ))
                    .or_default()
                    .push(ImportRecordMembership {
                        node_id: node_id.clone(),
                        suppressed: false,
                    });
            }
        }

        let mut import_records = grouped
            .into_iter()
            .map(|((source_id, source_label), memberships)| {
                let (record_id, imported_at_secs) = existing_record_meta
                    .get(&(source_id.clone(), source_label.clone()))
                    .cloned()
                    .unwrap_or_else(|| (format!("import-record:{}", source_id), imported_at_secs));
                ImportRecord {
                    record_id,
                    source_id,
                    source_label,
                    imported_at_secs,
                    memberships,
                }
            })
            .collect::<Vec<_>>();
        normalize_import_records(&mut import_records);
        self.import_records = import_records;
        self.sync_node_import_provenance_from_records();
    }

    pub(crate) fn set_node_import_provenance(
        &mut self,
        key: NodeKey,
        import_provenance: Vec<NodeImportProvenance>,
    ) -> bool {
        let node = match self.inner.node_weight_mut(key) {
            Some(node) => node,
            None => return false,
        };
        let mut normalized = import_provenance;
        normalized.sort();
        normalized.dedup();
        if node.import_provenance == normalized {
            return false;
        }
        node.import_provenance = normalized;
        self.rebuild_import_records_from_node_provenance(current_unix_timestamp_secs());
        true
    }

    pub(crate) fn set_node_tag_icon_override(
        &mut self,
        key: NodeKey,
        tag: &str,
        icon: Option<crate::graph::badge::BadgeIcon>,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if !node.tags.contains(tag) || tag.starts_with('#') || tag.starts_with("udc:") {
            return false;
        }
        match icon {
            Some(icon) => {
                if node.tag_presentation.icon_overrides.get(tag) == Some(&icon) {
                    return false;
                }
                node.tag_presentation
                    .icon_overrides
                    .insert(tag.to_string(), icon);
                true
            }
            None => node.tag_presentation.icon_overrides.remove(tag).is_some(),
        }
    }

    pub(crate) fn set_node_position(&mut self, key: NodeKey, position: Point2D<f32>) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.position == position && node.committed_position == position {
            return false;
        }
        node.position = position;
        node.committed_position = position;
        true
    }

    pub(crate) fn set_node_projected_position(
        &mut self,
        key: NodeKey,
        position: Point2D<f32>,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.position == position {
            return false;
        }
        node.position = position;
        true
    }

    pub fn node_projected_position(&self, key: NodeKey) -> Option<Point2D<f32>> {
        self.get_node(key).map(Node::projected_position)
    }

    pub fn node_committed_position(&self, key: NodeKey) -> Option<Point2D<f32>> {
        self.get_node(key).map(Node::committed_position)
    }

    pub fn projected_centroid(&self) -> Option<Point2D<f32>> {
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut count = 0.0f32;
        for (_, node) in self.nodes() {
            sum_x += node.position.x;
            sum_y += node.position.y;
            count += 1.0;
        }
        if count == 0.0 {
            None
        } else {
            Some(Point2D::new(sum_x / count, sum_y / count))
        }
    }

    pub(crate) fn set_node_form_draft(&mut self, key: NodeKey, form_draft: Option<String>) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.session_form_draft == form_draft {
            return false;
        }
        node.session_form_draft = form_draft;
        true
    }

    pub(crate) fn touch_node_last_visited_now(&mut self, key: NodeKey) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        node.last_visited = std::time::SystemTime::now();
        true
    }

    pub(crate) fn set_node_history_state(
        &mut self,
        key: NodeKey,
        history_entries: Vec<String>,
        history_index: usize,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        let clamped_index = if history_entries.is_empty() {
            0
        } else {
            history_index.min(history_entries.len() - 1)
        };
        if node.history_entries == history_entries && node.history_index == clamped_index {
            return false;
        }
        node.history_entries = history_entries;
        node.history_index = clamped_index;
        true
    }

    pub(crate) fn set_node_session_scroll(
        &mut self,
        key: NodeKey,
        session_scroll: Option<(f32, f32)>,
    ) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.session_scroll == session_scroll {
            return false;
        }
        node.session_scroll = session_scroll;
        true
    }

    pub(crate) fn set_node_lifecycle(&mut self, key: NodeKey, lifecycle: NodeLifecycle) -> bool {
        let Some(node) = self.inner.node_weight_mut(key) else {
            return false;
        };
        if node.lifecycle == lifecycle {
            return false;
        }
        node.lifecycle = lifecycle;
        true
    }

    /// Add an edge between two nodes
    pub(crate) fn add_edge(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        edge_type: EdgeType,
        label: Option<String>,
    ) -> Option<EdgeKey> {
        if !self.inner.contains_node(from) || !self.inner.contains_node(to) {
            return None;
        }
        if let Some(edge_key) = self.find_edge_key(from, to) {
            let payload = self.inner.edge_weight_mut(edge_key)?;
            return payload.add_edge_kind(edge_type, label).then_some(edge_key);
        }
        Some(
            self.inner
                .add_edge(from, to, EdgePayload::from_edge_type(edge_type, label)),
        )
    }

    pub(crate) fn assert_relation(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        assertion: EdgeAssertion,
    ) -> Option<EdgeKey> {
        if !self.inner.contains_node(from) || !self.inner.contains_node(to) {
            return None;
        }
        if let Some(edge_key) = self.find_edge_key(from, to) {
            let payload = self.inner.edge_weight_mut(edge_key)?;
            return payload.assert_relation(assertion).then_some(edge_key);
        }
        let mut payload = EdgePayload::new();
        if !payload.assert_relation(assertion) {
            return None;
        }
        Some(self.inner.add_edge(from, to, payload))
    }

    /// Replay helper: add node only if UUID is not already present.
    pub(crate) fn replay_add_node_with_id_if_missing(
        &mut self,
        id: Uuid,
        url: String,
        position: Point2D<f32>,
    ) -> Option<NodeKey> {
        if self.get_node_key_by_id(id).is_some() {
            return None;
        }
        Some(self.add_node_with_id(id, url, position))
    }

    /// Replay helper: add edge using stable node UUIDs.
    pub(crate) fn replay_add_edge_by_ids(
        &mut self,
        from_id: Uuid,
        to_id: Uuid,
        edge_type: EdgeType,
        label: Option<String>,
    ) -> Option<EdgeKey> {
        let from_key = self.get_node_key_by_id(from_id)?;
        let to_key = self.get_node_key_by_id(to_id)?;
        self.add_edge(from_key, to_key, edge_type, label)
    }

    /// Replay helper: remove node by stable UUID.
    pub(crate) fn replay_remove_node_by_id(&mut self, node_id: Uuid) -> bool {
        let Some(key) = self.get_node_key_by_id(node_id) else {
            return false;
        };
        self.remove_node(key)
    }

    /// Replay helper: remove edges between stable node UUIDs.
    pub(crate) fn replay_remove_edges_by_ids(
        &mut self,
        from_id: Uuid,
        to_id: Uuid,
        edge_type: EdgeType,
    ) -> usize {
        let Some(from_key) = self.get_node_key_by_id(from_id) else {
            return 0;
        };
        let Some(to_key) = self.get_node_key_by_id(to_id) else {
            return 0;
        };
        self.remove_edges(from_key, to_key, edge_type)
    }

    pub(crate) fn replay_retract_relations_by_ids(
        &mut self,
        from_id: Uuid,
        to_id: Uuid,
        selector: RelationSelector,
    ) -> usize {
        let Some(from_key) = self.get_node_key_by_id(from_id) else {
            return 0;
        };
        let Some(to_key) = self.get_node_key_by_id(to_id) else {
            return 0;
        };
        self.retract_relations(from_key, to_key, selector)
    }

    pub(crate) fn replay_assert_relation_by_ids(
        &mut self,
        from_id: Uuid,
        to_id: Uuid,
        assertion: EdgeAssertion,
    ) -> Option<EdgeKey> {
        let from_key = self.get_node_key_by_id(from_id)?;
        let to_key = self.get_node_key_by_id(to_id)?;
        self.assert_relation(from_key, to_key, assertion)
    }

    /// Dissolve helper: collect traversals from all incident edges and remove the node.
    pub(crate) fn dissolve_remove_node_collect_traversals(
        &mut self,
        key: NodeKey,
    ) -> Option<Vec<DissolvedTraversalRecord>> {
        let _ = self.get_node(key)?;

        let mut records = Vec::new();
        for edge in self
            .inner
            .edges_directed(key, Direction::Outgoing)
            .chain(self.inner.edges_directed(key, Direction::Incoming))
        {
            if edge.weight().traversals().is_empty() {
                continue;
            }

            let from_node = self.get_node(edge.source())?;
            let to_node = self.get_node(edge.target())?;
            records.push(DissolvedTraversalRecord {
                from_node_id: from_node.id,
                to_node_id: to_node.id,
                traversals: edge.weight().traversals().to_vec(),
            });
        }

        let _ = self.remove_node(key);
        Some(records)
    }

    /// Collect traversals from all incident edges without mutating graph state.
    pub(crate) fn collect_node_traversals(
        &self,
        key: NodeKey,
    ) -> Option<Vec<DissolvedTraversalRecord>> {
        let _ = self.get_node(key)?;

        let mut records = Vec::new();
        for edge in self
            .inner
            .edges_directed(key, Direction::Outgoing)
            .chain(self.inner.edges_directed(key, Direction::Incoming))
        {
            if edge.weight().traversals().is_empty() {
                continue;
            }

            let from_node = self.get_node(edge.source())?;
            let to_node = self.get_node(edge.target())?;
            records.push(DissolvedTraversalRecord {
                from_node_id: from_node.id,
                to_node_id: to_node.id,
                traversals: edge.weight().traversals().to_vec(),
            });
        }

        Some(records)
    }

    /// Dissolve helper: collect traversals for matching edges and remove them.
    pub(crate) fn dissolve_remove_edges_collect_traversals(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        edge_type: EdgeType,
    ) -> Option<(usize, Vec<DissolvedTraversalRecord>)> {
        if edge_type == EdgeType::History {
            let _ = self.get_node(from)?;
            let _ = self.get_node(to)?;
        }

        let from_node_id = self.get_node(from).map(|n| n.id);
        let to_node_id = self.get_node(to).map(|n| n.id);
        let mut records = Vec::new();

        if let (Some(from_node_id), Some(to_node_id)) = (from_node_id, to_node_id) {
            for edge in self.inner.edge_references().filter(|edge| {
                edge.source() == from
                    && edge.target() == to
                    && edge.weight().has_edge_type(edge_type)
            }) {
                if edge.weight().traversals().is_empty() {
                    continue;
                }

                records.push(DissolvedTraversalRecord {
                    from_node_id,
                    to_node_id,
                    traversals: edge.weight().traversals().to_vec(),
                });
            }
        }

        let removed = self.remove_edges(from, to, edge_type);
        Some((removed, records))
    }

    /// Collect traversals for matching edges without mutating graph state.
    pub(crate) fn collect_edge_traversals(
        &self,
        from: NodeKey,
        to: NodeKey,
        edge_type: EdgeType,
    ) -> Option<Vec<DissolvedTraversalRecord>> {
        if edge_type == EdgeType::History {
            let _ = self.get_node(from)?;
            let _ = self.get_node(to)?;
        }

        let from_node_id = self.get_node(from).map(|n| n.id);
        let to_node_id = self.get_node(to).map(|n| n.id);
        let mut records = Vec::new();

        if let (Some(from_node_id), Some(to_node_id)) = (from_node_id, to_node_id) {
            for edge in self.inner.edge_references().filter(|edge| {
                edge.source() == from
                    && edge.target() == to
                    && edge.weight().has_edge_type(edge_type)
            }) {
                if edge.weight().traversals().is_empty() {
                    continue;
                }

                records.push(DissolvedTraversalRecord {
                    from_node_id,
                    to_node_id,
                    traversals: edge.weight().traversals().to_vec(),
                });
            }
        }

        Some(records)
    }

    /// Remove all directed edges from `from` to `to` with the given type.
    /// Returns how many edges were removed.
    pub(crate) fn remove_edges(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        edge_type: EdgeType,
    ) -> usize {
        let edge_ids: Vec<EdgeKey> = self
            .inner
            .edge_references()
            .filter(|edge| {
                edge.source() == from
                    && edge.target() == to
                    && edge.weight().has_edge_type(edge_type)
            })
            .map(|edge| edge.id())
            .collect();

        let mut removed = 0usize;
        let mut edges_to_delete = Vec::new();
        for edge_id in edge_ids {
            if let Some(payload) = self.inner.edge_weight_mut(edge_id)
                && payload.remove_edge_type(edge_type)
            {
                removed += 1;
                if payload.is_empty() {
                    edges_to_delete.push(edge_id);
                }
            }
        }
        for edge_id in edges_to_delete {
            let _ = self.inner.remove_edge(edge_id);
        }
        removed
    }

    pub(crate) fn retract_relations(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        selector: RelationSelector,
    ) -> usize {
        let edge_ids: Vec<EdgeKey> = self
            .inner
            .edge_references()
            .filter(|edge| {
                edge.source() == from && edge.target() == to && edge.weight().has_relation(selector)
            })
            .map(|edge| edge.id())
            .collect();

        let mut removed = 0usize;
        let mut edges_to_delete = Vec::new();
        for edge_id in edge_ids {
            if let Some(payload) = self.inner.edge_weight_mut(edge_id)
                && payload.retract_relation(selector)
            {
                removed += 1;
                if payload.is_empty() {
                    edges_to_delete.push(edge_id);
                }
            }
        }
        for edge_id in edges_to_delete {
            let _ = self.inner.remove_edge(edge_id);
        }
        removed
    }

    /// Get a mutable edge payload by key.
    #[cfg(test)]
    pub(crate) fn get_edge_mut(&mut self, key: EdgeKey) -> Option<&mut EdgePayload> {
        self.inner.edge_weight_mut(key)
    }

    /// Get an edge payload by key.
    pub fn get_edge(&self, key: EdgeKey) -> Option<&EdgePayload> {
        self.inner.edge_weight(key)
    }

    /// Find the first directed edge key between two nodes.
    pub fn find_edge_key(&self, from: NodeKey, to: NodeKey) -> Option<EdgeKey> {
        self.inner.find_edge(from, to)
    }

    /// Append a traversal event to an existing edge, or create an edge carrying the traversal.
    pub(crate) fn push_traversal(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        traversal: Traversal,
    ) -> bool {
        if from == to || !self.inner.contains_node(from) || !self.inner.contains_node(to) {
            return false;
        }
        if let Some(edge_key) = self.find_edge_key(from, to)
            && let Some(payload) = self.inner.edge_weight_mut(edge_key)
        {
            payload.push_traversal(traversal);
            return true;
        }
        let mut payload = EdgePayload::new();
        payload.push_traversal(traversal);
        let _ = self.inner.add_edge(from, to, payload);
        true
    }

    /// Get a node by key
    pub fn get_node(&self, key: NodeKey) -> Option<&Node> {
        self.inner.node_weight(key)
    }

    /// Get a mutable node by key
    #[cfg(test)]
    pub(crate) fn get_node_mut(&mut self, key: NodeKey) -> Option<&mut Node> {
        self.inner.node_weight_mut(key)
    }

    /// Get a node and its key by URL
    pub fn get_node_by_url(&self, url: &str) -> Option<(NodeKey, &Node)> {
        let key = self.url_to_nodes.get(url)?.last().copied()?;
        Some((key, self.inner.node_weight(key)?))
    }

    /// Get all node keys currently mapped to a URL.
    pub fn get_nodes_by_url(&self, url: &str) -> Vec<NodeKey> {
        self.url_to_nodes.get(url).cloned().unwrap_or_default()
    }

    /// Get a node by UUID.
    pub fn get_node_by_id(&self, id: Uuid) -> Option<(NodeKey, &Node)> {
        let key = *self.id_to_node.get(&id)?;
        Some((key, self.inner.node_weight(key)?))
    }

    /// Get node key by UUID.
    pub fn get_node_key_by_id(&self, id: Uuid) -> Option<NodeKey> {
        self.id_to_node.get(&id).copied()
    }

    /// Iterate over all nodes as (key, node) pairs
    pub fn nodes(&self) -> impl Iterator<Item = (NodeKey, &Node)> {
        self.inner
            .node_indices()
            .map(move |idx| (idx, &self.inner[idx]))
    }

    /// Iterate over all edges as EdgeView
    pub fn edges(&self) -> impl Iterator<Item = EdgeView> + '_ {
        self.inner.edge_references().flat_map(|e| {
            let from = e.source();
            let to = e.target();
            let payload = e.weight();
            let mut out = Vec::with_capacity(6);
            if payload.has_kind(EdgeKind::Hyperlink) {
                out.push(EdgeView {
                    from,
                    to,
                    edge_type: EdgeType::Hyperlink,
                });
            }
            if payload.has_kind(EdgeKind::TraversalDerived) {
                out.push(EdgeView {
                    from,
                    to,
                    edge_type: EdgeType::History,
                });
            }
            if payload.has_kind(EdgeKind::UserGrouped) {
                out.push(EdgeView {
                    from,
                    to,
                    edge_type: EdgeType::UserGrouped,
                });
            }
            if let Some(arrangement) = payload.arrangement_data() {
                for sub_kind in &arrangement.sub_kinds {
                    out.push(EdgeView {
                        from,
                        to,
                        edge_type: EdgeType::ArrangementRelation(*sub_kind),
                    });
                }
            }
            if let Some(containment) = payload.containment_data() {
                for sub_kind in &containment.sub_kinds {
                    out.push(EdgeView {
                        from,
                        to,
                        edge_type: EdgeType::ContainmentRelation(*sub_kind),
                    });
                }
            }
            if payload.has_kind(EdgeKind::ImportedRelation) {
                out.push(EdgeView {
                    from,
                    to,
                    edge_type: EdgeType::ImportedRelation,
                });
            }
            if payload.has_kind(EdgeKind::AgentDerived) {
                out.push(EdgeView {
                    from,
                    to,
                    // decay_progress will be populated from EdgePayload data when AgentDerived
                    // payload storage is implemented; 0.0 = freshly asserted in the interim.
                    edge_type: EdgeType::AgentDerived {
                        decay_progress: 0.0,
                    },
                });
            }
            out.into_iter()
        })
    }

    pub fn arrangement_edges(&self) -> impl Iterator<Item = ArrangementEdgeView> + '_ {
        self.inner.edge_references().flat_map(|e| {
            let from = e.source();
            let to = e.target();
            e.weight()
                .arrangement_data()
                .map(|data| {
                    data.sub_kinds
                        .iter()
                        .copied()
                        .map(move |sub_kind| ArrangementEdgeView { from, to, sub_kind })
                })
                .into_iter()
                .flatten()
        })
    }

    pub fn containment_edges(&self) -> impl Iterator<Item = ContainmentEdgeView> + '_ {
        self.inner.edge_references().flat_map(|e| {
            let from = e.source();
            let to = e.target();
            e.weight()
                .containment_data()
                .map(|data| {
                    data.sub_kinds
                        .iter()
                        .copied()
                        .map(move |sub_kind| ContainmentEdgeView { from, to, sub_kind })
                })
                .into_iter()
                .flatten()
        })
    }

    /// Rebuild derived containment edges from current node URLs.
    ///
    /// Derived relations are additive/read-only and are never persisted.
    pub(crate) fn rebuild_derived_containment_relations(&mut self) {
        let edge_ids: Vec<EdgeKey> = self.inner.edge_indices().collect();
        let mut empty_edges = Vec::new();
        for edge_id in edge_ids {
            if let Some(payload) = self.inner.edge_weight_mut(edge_id) {
                let mut removed_any = false;
                removed_any |= payload
                    .remove_edge_type(EdgeType::ContainmentRelation(ContainmentSubKind::UrlPath));
                removed_any |= payload
                    .remove_edge_type(EdgeType::ContainmentRelation(ContainmentSubKind::Domain));
                if removed_any && payload.is_empty() {
                    empty_edges.push(edge_id);
                }
            }
        }
        for edge_id in empty_edges {
            let _ = self.inner.remove_edge(edge_id);
        }

        let mut domain_anchor: HashMap<String, (NodeKey, usize, Uuid)> = HashMap::new();
        for (key, node) in self.nodes() {
            let Ok(parsed) = url::Url::parse(&node.url) else {
                continue;
            };
            let depth = parsed.path_segments().map_or(0, |segments| {
                segments.filter(|segment| !segment.is_empty()).count()
            });

            let Some(host) = parsed.host_str() else {
                continue;
            };
            let host = host.to_ascii_lowercase();
            let candidate = (key, depth, node.id);
            domain_anchor
                .entry(host)
                .and_modify(|current| {
                    if candidate.1 < current.1
                        || (candidate.1 == current.1 && candidate.2 < current.2)
                    {
                        *current = candidate;
                    }
                })
                .or_insert(candidate);
        }

        let mut url_parent_edges = Vec::new();
        let mut domain_edges = Vec::new();
        for (key, node) in self.nodes() {
            let Ok(parsed) = url::Url::parse(&node.url) else {
                continue;
            };

            if let Some(parent_url) = containment_parent_url(&parsed)
                && let Some((parent_key, _)) = self.get_node_by_url(&parent_url)
                && parent_key != key
            {
                url_parent_edges.push((key, parent_key));
            }

            if let Some(host) = parsed.host_str() {
                let host = host.to_ascii_lowercase();
                if let Some((anchor_key, _, _)) = domain_anchor.get(&host)
                    && *anchor_key != key
                {
                    domain_edges.push((key, *anchor_key));
                }
            }
        }

        for (from, to) in url_parent_edges {
            let _ = self.add_edge(
                from,
                to,
                EdgeType::ContainmentRelation(ContainmentSubKind::UrlPath),
                None,
            );
        }
        for (from, to) in domain_edges {
            let _ = self.add_edge(
                from,
                to,
                EdgeType::ContainmentRelation(ContainmentSubKind::Domain),
                None,
            );
        }
    }

    /// Iterate outgoing neighbor keys for a node
    pub fn out_neighbors(&self, key: NodeKey) -> impl Iterator<Item = NodeKey> + '_ {
        self.inner.neighbors_directed(key, Direction::Outgoing)
    }

    /// Iterate incoming neighbor keys for a node
    pub fn in_neighbors(&self, key: NodeKey) -> impl Iterator<Item = NodeKey> + '_ {
        self.inner.neighbors_directed(key, Direction::Incoming)
    }

    /// Iterate undirected neighbor keys for a node.
    pub fn neighbors_undirected(&self, key: NodeKey) -> impl Iterator<Item = NodeKey> + '_ {
        self.inner.neighbors_undirected(key)
    }

    /// Undirected neighbors sorted by stable node-key order.
    pub fn neighbors_undirected_sorted(&self, key: NodeKey) -> Vec<NodeKey> {
        let mut neighbors: Vec<NodeKey> = self
            .neighbors_undirected(key)
            .filter(|neighbor| *neighbor != key && self.get_node(*neighbor).is_some())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        neighbors.sort_by_key(|neighbor| neighbor.index());
        neighbors
    }

    /// Seed nodes plus one-hop undirected neighbors for frame import workflows.
    pub fn connected_frame_import_nodes(&self, seeds: &[NodeKey]) -> Vec<NodeKey> {
        let mut out = HashSet::new();
        for seed in seeds {
            if self.get_node(*seed).is_none() {
                continue;
            }
            out.insert(*seed);
            out.extend(self.neighbors_undirected(*seed));
        }
        let mut nodes: Vec<NodeKey> = out
            .into_iter()
            .filter(|key| self.get_node(*key).is_some())
            .collect();
        nodes.sort_by_key(|key| key.index());
        nodes
    }

    /// Connected candidate expansion around a source node with depth annotations.
    ///
    /// `max_depth` currently supports 1 or 2, matching connected-open scope behavior.
    pub fn connected_candidates_with_depth(
        &self,
        source: NodeKey,
        max_depth: u8,
    ) -> Vec<(NodeKey, u8)> {
        if self.get_node(source).is_none() || max_depth == 0 {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut visited = HashSet::from([source]);

        let depth1 = self.neighbors_undirected_sorted(source);
        for neighbor in depth1 {
            if visited.insert(neighbor) {
                out.push((neighbor, 1));
            }
        }

        if max_depth < 2 {
            return out;
        }

        let depth1_nodes: Vec<NodeKey> = out
            .iter()
            .filter_map(|(node, depth)| (*depth == 1).then_some(*node))
            .collect();
        for depth1_node in depth1_nodes {
            for neighbor in self.neighbors_undirected_sorted(depth1_node) {
                if visited.insert(neighbor) {
                    out.push((neighbor, 2));
                }
            }
        }

        out
    }

    /// Undirected hop distances from `source` using unit edge weights.
    pub fn hop_distances_from(&self, source: NodeKey) -> HashMap<NodeKey, usize> {
        if self.get_node(source).is_none() {
            return HashMap::new();
        }
        dijkstra(&UndirectedAdaptor(&self.inner), source, None, |_| 1_usize)
            .into_iter()
            .collect()
    }

    /// Nodes with no incoming or outgoing edges.
    pub fn orphan_node_keys(&self) -> Vec<NodeKey> {
        self.inner
            .node_indices()
            .filter(|&key| {
                self.inner
                    .edges_directed(key, Direction::Outgoing)
                    .next()
                    .is_none()
                    && self
                        .inner
                        .edges_directed(key, Direction::Incoming)
                        .next()
                        .is_none()
            })
            .collect()
    }

    /// Shortest undirected path between two nodes using unit edge weights.
    pub fn shortest_path(&self, from: NodeKey, to: NodeKey) -> Option<Vec<NodeKey>> {
        if self.get_node(from).is_none() || self.get_node(to).is_none() {
            return None;
        }
        astar(
            &UndirectedAdaptor(&self.inner),
            from,
            |node| node == to,
            |_| 1_usize,
            |_| 0_usize,
        )
        .map(|(_, path)| path)
    }

    /// Reachability in the undirected graph.
    pub fn is_reachable(&self, from: NodeKey, to: NodeKey) -> bool {
        if self.get_node(from).is_none() || self.get_node(to).is_none() {
            return false;
        }
        has_path_connecting(&UndirectedAdaptor(&self.inner), from, to, None)
    }

    /// Weakly connected components (undirected projection).
    pub fn weakly_connected_components(&self) -> Vec<Vec<NodeKey>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();
        for start in self.inner.node_indices() {
            if !visited.insert(start) {
                continue;
            }
            let mut component = Vec::new();
            let mut stack = vec![start];
            while let Some(current) = stack.pop() {
                component.push(current);
                for neighbor in self.neighbors_undirected(current) {
                    if visited.insert(neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
            components.push(component);
        }
        components
    }

    /// Strongly connected components in the directed graph.
    pub fn strongly_connected_components(&self) -> Vec<Vec<NodeKey>> {
        kosaraju_scc(&self.inner)
    }

    /// Check if a directed edge exists from `from` to `to`
    pub fn has_edge_between(&self, from: NodeKey, to: NodeKey) -> bool {
        self.inner.find_edge(from, to).is_some()
    }

    /// Count of nodes in the graph
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Count of edges in the graph
    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Serialize the graph to a persistable snapshot
    pub fn to_snapshot(&self) -> GraphSnapshot {
        let nodes = self
            .nodes()
            .map(|(_, node)| PersistedNode {
                node_id: node.id.to_string(),
                url: node.url.clone(),
                cached_host: node.cached_host.clone(),
                title: node.title.clone(),
                position_x: node.committed_position.x,
                position_y: node.committed_position.y,
                tags: {
                    let mut tags = node.tags.iter().cloned().collect::<Vec<_>>();
                    tags.sort();
                    tags
                },
                tag_presentation: node.tag_presentation.clone(),
                import_provenance: node.import_provenance.clone(),
                is_pinned: node.is_pinned,
                history_entries: node.history_entries.clone(),
                history_index: node.history_index,
                thumbnail_png: node.thumbnail_png.clone(),
                thumbnail_width: node.thumbnail_width,
                thumbnail_height: node.thumbnail_height,
                favicon_rgba: node.favicon_rgba.clone(),
                favicon_width: node.favicon_width,
                favicon_height: node.favicon_height,
                session_state: Some(PersistedNodeSessionState {
                    history_entries: node.history_entries.clone(),
                    history_index: node.history_index,
                    scroll_x: node.session_scroll.map(|(x, _)| x),
                    scroll_y: node.session_scroll.map(|(_, y)| y),
                    form_draft: node.session_form_draft.clone(),
                }),
                classifications: node.classifications.clone(),
                mime_hint: node.mime_hint.clone(),
                address_kind: match node.address_kind {
                    AddressKind::Http => PersistedAddressKind::Http,
                    AddressKind::File => PersistedAddressKind::File,
                    AddressKind::Data => PersistedAddressKind::Data,
                    AddressKind::GraphshellClip => PersistedAddressKind::GraphshellClip,
                    AddressKind::Directory => PersistedAddressKind::Directory,
                    AddressKind::Unknown => PersistedAddressKind::Unknown,
                },
                frame_layout_hints: node.frame_layout_hints.clone(),
                frame_split_offer_suppressed: node.frame_split_offer_suppressed,
            })
            .collect();

        let edges = self
            .inner
            .edge_references()
            .map(|edge| {
                let from_node_id = self
                    .get_node(edge.source())
                    .map(|n| n.id.to_string())
                    .unwrap_or_default();
                let to_node_id = self
                    .get_node(edge.target())
                    .map(|n| n.id.to_string())
                    .unwrap_or_default();
                let payload = edge.weight();
                PersistedEdge {
                    from_node_id,
                    to_node_id,
                    families: payload
                        .families()
                        .iter()
                        .map(|family| match family {
                            EdgeFamily::Semantic => PersistedEdgeFamily::Semantic,
                            EdgeFamily::Traversal => PersistedEdgeFamily::Traversal,
                            EdgeFamily::Containment => PersistedEdgeFamily::Containment,
                            EdgeFamily::Arrangement => PersistedEdgeFamily::Arrangement,
                            EdgeFamily::Imported => PersistedEdgeFamily::Imported,
                            EdgeFamily::Provenance => PersistedEdgeFamily::Provenance,
                        })
                        .collect(),
                    semantic: Some(PersistedSemanticEdgeData {
                        sub_kinds: [
                            payload
                                .has_edge_type(EdgeType::Hyperlink)
                                .then_some(PersistedSemanticSubKind::Hyperlink),
                            payload
                                .has_edge_type(EdgeType::UserGrouped)
                                .then_some(PersistedSemanticSubKind::UserGrouped),
                            payload
                                .has_edge_type(EdgeType::AgentDerived {
                                    decay_progress: 0.0,
                                })
                                .then_some(PersistedSemanticSubKind::AgentDerived),
                        ]
                        .into_iter()
                        .flatten()
                        .collect(),
                        label: payload.label().map(str::to_string),
                        agent_decay_progress: payload
                            .has_edge_type(EdgeType::AgentDerived {
                                decay_progress: 0.0,
                            })
                            .then_some(0.0),
                    })
                    .filter(|data| !data.sub_kinds.is_empty() || data.label.is_some()),
                    traversal: payload
                        .traversal_data()
                        .map(|data| PersistedTraversalEdgeData {
                            traversals: data
                                .traversals
                                .iter()
                                .map(|traversal| PersistedTraversalRecord {
                                    timestamp_ms: traversal.timestamp_ms,
                                    trigger: match traversal.trigger {
                                        NavigationTrigger::Unknown => {
                                            PersistedNavigationTrigger::Unknown
                                        }
                                        NavigationTrigger::LinkClick => {
                                            PersistedNavigationTrigger::LinkClick
                                        }
                                        NavigationTrigger::Back => PersistedNavigationTrigger::Back,
                                        NavigationTrigger::Forward => {
                                            PersistedNavigationTrigger::Forward
                                        }
                                        NavigationTrigger::AddressBarEntry => {
                                            PersistedNavigationTrigger::AddressBarEntry
                                        }
                                        NavigationTrigger::PanePromotion => {
                                            PersistedNavigationTrigger::PanePromotion
                                        }
                                        NavigationTrigger::Programmatic => {
                                            PersistedNavigationTrigger::Programmatic
                                        }
                                    },
                                })
                                .collect(),
                            metrics: PersistedTraversalMetrics {
                                total_navigations: data.metrics.total_navigations,
                                forward_navigations: data.metrics.forward_navigations,
                                backward_navigations: data.metrics.backward_navigations,
                                last_navigated_at: data.metrics.last_navigated_at,
                            },
                        }),
                    containment: payload.containment_data().map(|data| {
                        PersistedContainmentEdgeData {
                            sub_kinds: data
                                .sub_kinds
                                .iter()
                                .map(|sub_kind| match sub_kind {
                                    ContainmentSubKind::UrlPath => {
                                        PersistedContainmentSubKind::UrlPath
                                    }
                                    ContainmentSubKind::Domain => {
                                        PersistedContainmentSubKind::Domain
                                    }
                                    ContainmentSubKind::FileSystem => {
                                        PersistedContainmentSubKind::FileSystem
                                    }
                                    ContainmentSubKind::UserFolder => {
                                        PersistedContainmentSubKind::UserFolder
                                    }
                                    ContainmentSubKind::ClipSource => {
                                        PersistedContainmentSubKind::ClipSource
                                    }
                                    ContainmentSubKind::NotebookSection => {
                                        PersistedContainmentSubKind::NotebookSection
                                    }
                                    ContainmentSubKind::CollectionMember => {
                                        PersistedContainmentSubKind::CollectionMember
                                    }
                                })
                                .collect(),
                        }
                    }),
                    arrangement: payload.arrangement_data().map(|data| {
                        PersistedArrangementEdgeData {
                            sub_kinds: data
                                .sub_kinds
                                .iter()
                                .copied()
                                .filter(|sub_kind| {
                                    sub_kind.durability() == RelationDurability::Durable
                                })
                                .map(|sub_kind| match sub_kind {
                                    ArrangementSubKind::FrameMember => {
                                        PersistedArrangementSubKind::FrameMember
                                    }
                                    ArrangementSubKind::TileGroup => {
                                        PersistedArrangementSubKind::TileGroup
                                    }
                                    ArrangementSubKind::SplitPair => {
                                        PersistedArrangementSubKind::SplitPair
                                    }
                                })
                                .collect(),
                        }
                    }),
                    imported: payload
                        .imported_data()
                        .map(|data| PersistedImportedEdgeData {
                            sub_kinds: data
                                .sub_kinds
                                .iter()
                                .map(|sub_kind| match sub_kind {
                                    ImportedSubKind::BookmarkFolder => {
                                        PersistedImportedSubKind::BookmarkFolder
                                    }
                                    ImportedSubKind::HistoryImport => {
                                        PersistedImportedSubKind::HistoryImport
                                    }
                                    ImportedSubKind::RssMembership => {
                                        PersistedImportedSubKind::RssMembership
                                    }
                                    ImportedSubKind::FileSystemImport => {
                                        PersistedImportedSubKind::FileSystemImport
                                    }
                                    ImportedSubKind::ArchiveMembership => {
                                        PersistedImportedSubKind::ArchiveMembership
                                    }
                                    ImportedSubKind::SharedCollection => {
                                        PersistedImportedSubKind::SharedCollection
                                    }
                                })
                                .collect(),
                        }),
                    provenance: payload
                        .provenance_data()
                        .map(|data| PersistedProvenanceEdgeData {
                            sub_kinds: data
                                .sub_kinds
                                .iter()
                                .map(|sub_kind| match sub_kind {
                                    ProvenanceSubKind::ClippedFrom => {
                                        PersistedProvenanceSubKind::ClippedFrom
                                    }
                                    ProvenanceSubKind::ExcerptedFrom => {
                                        PersistedProvenanceSubKind::ExcerptedFrom
                                    }
                                    ProvenanceSubKind::SummarizedFrom => {
                                        PersistedProvenanceSubKind::SummarizedFrom
                                    }
                                    ProvenanceSubKind::TranslatedFrom => {
                                        PersistedProvenanceSubKind::TranslatedFrom
                                    }
                                    ProvenanceSubKind::RewrittenFrom => {
                                        PersistedProvenanceSubKind::RewrittenFrom
                                    }
                                    ProvenanceSubKind::GeneratedFrom => {
                                        PersistedProvenanceSubKind::GeneratedFrom
                                    }
                                    ProvenanceSubKind::ExtractedFrom => {
                                        PersistedProvenanceSubKind::ExtractedFrom
                                    }
                                    ProvenanceSubKind::ImportedFromSource => {
                                        PersistedProvenanceSubKind::ImportedFromSource
                                    }
                                })
                                .collect(),
                        }),
                }
            })
            .collect();

        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        GraphSnapshot {
            nodes,
            edges,
            import_records: self.import_records.clone(),
            timestamp_secs,
        }
    }

    /// Rebuild a graph from a persisted snapshot
    pub fn from_snapshot(snapshot: &GraphSnapshot) -> Self {
        let mut graph = Graph::new();

        for pnode in &snapshot.nodes {
            let Ok(node_id) = Uuid::parse_str(&pnode.node_id) else {
                continue;
            };
            let key = graph.add_node_with_id(
                node_id,
                pnode.url.clone(),
                Point2D::new(pnode.position_x, pnode.position_y),
            );
            let mut restore_url_from_session: Option<String> = None;
            if let Some(node) = graph.inner.node_weight_mut(key) {
                node.title = pnode.title.clone();
                node.cached_host = pnode
                    .cached_host
                    .clone()
                    .or_else(|| cached_host_from_url(&node.url));
                node.tags = pnode.tags.iter().cloned().collect();
                node.tag_presentation = pnode.tag_presentation.clone();
                node.import_provenance = pnode.import_provenance.clone();
                node.classifications = pnode.classifications.clone();
                node.is_pinned = pnode.is_pinned;
                node.history_entries = pnode.history_entries.clone();
                node.history_index = pnode
                    .history_index
                    .min(node.history_entries.len().saturating_sub(1));
                node.thumbnail_png = pnode.thumbnail_png.clone();
                node.thumbnail_width = pnode.thumbnail_width;
                node.thumbnail_height = pnode.thumbnail_height;
                node.favicon_rgba = pnode.favicon_rgba.clone();
                node.favicon_width = pnode.favicon_width;
                node.favicon_height = pnode.favicon_height;
                node.mime_hint = pnode.mime_hint.clone();
                node.address_kind = match pnode.address_kind {
                    PersistedAddressKind::Http => AddressKind::Http,
                    PersistedAddressKind::File => AddressKind::File,
                    PersistedAddressKind::Data => AddressKind::Data,
                    PersistedAddressKind::GraphshellClip => AddressKind::GraphshellClip,
                    PersistedAddressKind::Directory => AddressKind::Directory,
                    PersistedAddressKind::Unknown => AddressKind::Unknown,
                };
                node.frame_layout_hints = pnode.frame_layout_hints.clone();
                node.frame_split_offer_suppressed = pnode.frame_split_offer_suppressed;
                if let Some(session) = &pnode.session_state {
                    node.history_entries = session.history_entries.clone();
                    node.history_index = session
                        .history_index
                        .min(node.history_entries.len().saturating_sub(1));
                    restore_url_from_session =
                        node.history_entries.get(node.history_index).cloned();
                    node.session_scroll = session.scroll_x.zip(session.scroll_y);
                    node.session_form_draft = session.form_draft.clone();
                }
            }
            if let Some(current_url) = restore_url_from_session
                && !current_url.is_empty()
            {
                // Recompute MIME hint and address kind from the restored URL.
                if let Some(node) = graph.inner.node_weight_mut(key) {
                    node.mime_hint = detect_mime(&current_url, None);
                    node.address_kind = address_kind_from_url(&current_url);
                }
                let _ = graph.update_node_url(key, current_url);
            }
        }

        for pedge in &snapshot.edges {
            let from_key = Uuid::parse_str(&pedge.from_node_id)
                .ok()
                .and_then(|id| graph.get_node_key_by_id(id));
            let to_key = Uuid::parse_str(&pedge.to_node_id)
                .ok()
                .and_then(|id| graph.get_node_key_by_id(id));
            if let (Some(from), Some(to)) = (from_key, to_key) {
                if let Some(semantic) = &pedge.semantic {
                    for sub_kind in &semantic.sub_kinds {
                        let edge_type = match sub_kind {
                            PersistedSemanticSubKind::Hyperlink => EdgeType::Hyperlink,
                            PersistedSemanticSubKind::UserGrouped => EdgeType::UserGrouped,
                            PersistedSemanticSubKind::AgentDerived => EdgeType::AgentDerived {
                                decay_progress: semantic.agent_decay_progress.unwrap_or(0.0),
                            },
                            _ => continue,
                        };
                        let _ = graph.add_edge(from, to, edge_type, semantic.label.clone());
                    }
                }
                if let Some(arrangement) = &pedge.arrangement {
                    for sub_kind in &arrangement.sub_kinds {
                        let edge_type = match sub_kind {
                            PersistedArrangementSubKind::FrameMember => {
                                EdgeType::ArrangementRelation(ArrangementSubKind::FrameMember)
                            }
                            PersistedArrangementSubKind::TileGroup => {
                                EdgeType::ArrangementRelation(ArrangementSubKind::TileGroup)
                            }
                            PersistedArrangementSubKind::SplitPair => {
                                EdgeType::ArrangementRelation(ArrangementSubKind::SplitPair)
                            }
                            PersistedArrangementSubKind::TabNeighbor
                            | PersistedArrangementSubKind::ActiveTab
                            | PersistedArrangementSubKind::PinnedInFrame => continue,
                        };
                        let _ = graph.add_edge(from, to, edge_type, None);
                    }
                }
                if let Some(containment) = &pedge.containment {
                    for sub_kind in &containment.sub_kinds {
                        let edge_type = match sub_kind {
                            PersistedContainmentSubKind::UrlPath => {
                                EdgeType::ContainmentRelation(ContainmentSubKind::UrlPath)
                            }
                            PersistedContainmentSubKind::Domain => {
                                EdgeType::ContainmentRelation(ContainmentSubKind::Domain)
                            }
                            _ => continue,
                        };
                        let _ = graph.add_edge(from, to, edge_type, None);
                    }
                }
                if let Some(imported) = &pedge.imported {
                    for sub_kind in &imported.sub_kinds {
                        let assertion = match sub_kind {
                            PersistedImportedSubKind::BookmarkFolder => EdgeAssertion::Imported {
                                sub_kind: ImportedSubKind::BookmarkFolder,
                            },
                            PersistedImportedSubKind::HistoryImport => EdgeAssertion::Imported {
                                sub_kind: ImportedSubKind::HistoryImport,
                            },
                            PersistedImportedSubKind::RssMembership => EdgeAssertion::Imported {
                                sub_kind: ImportedSubKind::RssMembership,
                            },
                            PersistedImportedSubKind::FileSystemImport => EdgeAssertion::Imported {
                                sub_kind: ImportedSubKind::FileSystemImport,
                            },
                            PersistedImportedSubKind::ArchiveMembership => {
                                EdgeAssertion::Imported {
                                    sub_kind: ImportedSubKind::ArchiveMembership,
                                }
                            }
                            PersistedImportedSubKind::SharedCollection => EdgeAssertion::Imported {
                                sub_kind: ImportedSubKind::SharedCollection,
                            },
                        };
                        let _ = graph.assert_relation(from, to, assertion);
                    }
                }
                if let Some(provenance) = &pedge.provenance {
                    for sub_kind in &provenance.sub_kinds {
                        let assertion = match sub_kind {
                            PersistedProvenanceSubKind::ClippedFrom => EdgeAssertion::Provenance {
                                sub_kind: ProvenanceSubKind::ClippedFrom,
                            },
                            PersistedProvenanceSubKind::ExcerptedFrom => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::ExcerptedFrom,
                                }
                            }
                            PersistedProvenanceSubKind::SummarizedFrom => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::SummarizedFrom,
                                }
                            }
                            PersistedProvenanceSubKind::TranslatedFrom => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::TranslatedFrom,
                                }
                            }
                            PersistedProvenanceSubKind::RewrittenFrom => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::RewrittenFrom,
                                }
                            }
                            PersistedProvenanceSubKind::GeneratedFrom => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::GeneratedFrom,
                                }
                            }
                            PersistedProvenanceSubKind::ExtractedFrom => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::ExtractedFrom,
                                }
                            }
                            PersistedProvenanceSubKind::ImportedFromSource => {
                                EdgeAssertion::Provenance {
                                    sub_kind: ProvenanceSubKind::ImportedFromSource,
                                }
                            }
                        };
                        let _ = graph.assert_relation(from, to, assertion);
                    }
                }
                if let Some(traversal) = &pedge.traversal {
                    let _ = graph.add_edge(from, to, EdgeType::History, None);
                    if let Some(edge_key) = graph.find_edge_key(from, to)
                        && let Some(payload) = graph.inner.edge_weight_mut(edge_key)
                        && let Some(data) = payload.traversal.as_mut()
                    {
                        data.traversals = traversal
                            .traversals
                            .iter()
                            .map(|record| Traversal {
                                timestamp_ms: record.timestamp_ms,
                                trigger: match record.trigger {
                                    PersistedNavigationTrigger::Unknown => {
                                        NavigationTrigger::Unknown
                                    }
                                    PersistedNavigationTrigger::LinkClick => {
                                        NavigationTrigger::LinkClick
                                    }
                                    PersistedNavigationTrigger::Back => NavigationTrigger::Back,
                                    PersistedNavigationTrigger::Forward => {
                                        NavigationTrigger::Forward
                                    }
                                    PersistedNavigationTrigger::AddressBarEntry => {
                                        NavigationTrigger::AddressBarEntry
                                    }
                                    PersistedNavigationTrigger::PanePromotion => {
                                        NavigationTrigger::PanePromotion
                                    }
                                    PersistedNavigationTrigger::Programmatic => {
                                        NavigationTrigger::Programmatic
                                    }
                                },
                            })
                            .collect();
                        data.metrics = EdgeMetrics {
                            total_navigations: traversal.metrics.total_navigations,
                            forward_navigations: traversal.metrics.forward_navigations,
                            backward_navigations: traversal.metrics.backward_navigations,
                            last_navigated_at: traversal.metrics.last_navigated_at,
                        };
                    }
                }
            }
        }

        if snapshot.import_records.is_empty() {
            graph.rebuild_import_records_from_node_provenance(snapshot.timestamp_secs);
        } else {
            graph.import_records = snapshot.import_records.clone();
            normalize_import_records(&mut graph.import_records);
            graph.sync_node_import_provenance_from_records();
        }

        graph.rebuild_derived_containment_relations();

        graph
    }

    fn remove_url_mapping(&mut self, url: &str, key: NodeKey) {
        if let Some(keys) = self.url_to_nodes.get_mut(url) {
            keys.retain(|candidate| *candidate != key);
            if keys.is_empty() {
                self.url_to_nodes.remove(url);
            }
        }
    }
}

fn containment_parent_url(url: &url::Url) -> Option<String> {
    if !matches!(url.scheme(), "http" | "https" | "file") {
        return None;
    }

    let mut parent = url.clone();
    parent.set_query(None);
    parent.set_fragment(None);

    let mut segments: Vec<String> = parent
        .path_segments()
        .map(|parts| {
            parts
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if segments.is_empty() {
        return None;
    }
    segments.pop();

    let parent_path = if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}/", segments.join("/"))
    };

    parent.set_path(&parent_path);
    Some(parent.to_string())
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Archive for Graph {
    type Archived = Archived<GraphSnapshot>;
    type Resolver = Resolver<GraphSnapshot>;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let snapshot = self.to_snapshot();
        snapshot.resolve(resolver, out);
    }
}

impl<S> Serialize<S> for Graph
where
    S: Fallible + ?Sized,
    GraphSnapshot: Serialize<S>,
{
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        let snapshot = self.to_snapshot();
        snapshot.serialize(serializer)
    }
}

impl<D> Deserialize<Graph, D> for Archived<GraphSnapshot>
where
    D: Fallible + ?Sized,
    Archived<GraphSnapshot>: Deserialize<GraphSnapshot, D>,
{
    fn deserialize(&self, deserializer: &mut D) -> Result<Graph, D::Error> {
        let snapshot = <Archived<GraphSnapshot> as Deserialize<GraphSnapshot, D>>::deserialize(
            self,
            deserializer,
        )?;
        Ok(Graph::from_snapshot(&snapshot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_new() {
        let graph = Graph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = Graph::new();
        let pos = Point2D::new(100.0, 200.0);
        let key = graph.add_node("https://example.com".to_string(), pos);

        let node = graph.get_node(key).unwrap();
        assert_eq!(node.url, "https://example.com");
        assert_eq!(node.title, "https://example.com");
        assert_eq!(node.position.x, 100.0);
        assert_eq!(node.position.y, 200.0);
        assert_eq!(node.committed_position.x, 100.0);
        assert_eq!(node.committed_position.y, 200.0);
        assert_eq!(node.velocity.x, 0.0);
        assert_eq!(node.velocity.y, 0.0);
        assert!(!node.is_pinned);
        assert_eq!(node.lifecycle, NodeLifecycle::Cold);
    }

    #[test]
    fn test_add_multiple_nodes() {
        let mut graph = Graph::new();
        let key1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let key2 = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));
        let key3 = graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 2.0));

        assert_eq!(graph.node_count(), 3);
        assert!(graph.get_node(key1).is_some());
        assert!(graph.get_node(key2).is_some());
        assert!(graph.get_node(key3).is_some());
    }

    #[test]
    fn test_duplicate_url_nodes_have_distinct_ids() {
        let mut graph = Graph::new();
        let key1 = graph.add_node("https://same.com".to_string(), Point2D::new(0.0, 0.0));
        let key2 = graph.add_node("https://same.com".to_string(), Point2D::new(10.0, 10.0));

        assert_ne!(key1, key2);
        let node1 = graph.get_node(key1).unwrap();
        let node2 = graph.get_node(key2).unwrap();
        assert_ne!(node1.id, node2.id);
        assert_eq!(graph.get_nodes_by_url("https://same.com").len(), 2);
    }

    #[test]
    fn test_get_node_by_url() {
        let mut graph = Graph::new();
        graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        let (_, node) = graph.get_node_by_url("https://example.com").unwrap();
        assert_eq!(node.url, "https://example.com");

        assert!(graph.get_node_by_url("https://notfound.com").is_none());
    }

    #[test]
    fn test_get_node_mut() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        {
            let node = graph.get_node_mut(key).unwrap();
            node.position = Point2D::new(100.0, 200.0);
            node.is_pinned = true;
        }

        let node = graph.get_node(key).unwrap();
        assert_eq!(node.position.x, 100.0);
        assert_eq!(node.position.y, 200.0);
        assert!(node.is_pinned);
    }

    #[test]
    fn test_projected_position_does_not_change_committed_snapshot_position() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(10.0, 20.0));

        assert!(graph.set_node_projected_position(key, Point2D::new(150.0, 250.0)));

        let node = graph.get_node(key).unwrap();
        assert_eq!(node.position, Point2D::new(150.0, 250.0));
        assert_eq!(node.committed_position, Point2D::new(10.0, 20.0));

        let snapshot = graph.to_snapshot();
        assert_eq!(snapshot.nodes[0].position_x, 10.0);
        assert_eq!(snapshot.nodes[0].position_y, 20.0);
    }

    #[test]
    fn test_projected_helpers_expose_projected_and_committed_positions() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(10.0, 20.0));

        assert!(graph.set_node_projected_position(key, Point2D::new(40.0, 60.0)));

        assert_eq!(
            graph.node_projected_position(key),
            Some(Point2D::new(40.0, 60.0))
        );
        assert_eq!(
            graph.node_committed_position(key),
            Some(Point2D::new(10.0, 20.0))
        );
        assert_eq!(graph.projected_centroid(), Some(Point2D::new(40.0, 60.0)));
    }

    #[test]
    fn test_add_edge() {
        let mut graph = Graph::new();
        let node1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let node2 = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));

        graph
            .add_edge(node1, node2, EdgeType::Hyperlink, None)
            .unwrap();

        // Check adjacency via graph methods
        assert!(graph.has_edge_between(node1, node2));
        assert!(!graph.has_edge_between(node2, node1));
        assert_eq!(graph.out_neighbors(node1).count(), 1);
        assert_eq!(graph.in_neighbors(node2).count(), 1);
    }

    #[test]
    fn test_add_edge_invalid_nodes() {
        let mut graph = Graph::new();
        let node1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));

        let invalid_key = NodeIndex::new(999);

        assert!(
            graph
                .add_edge(invalid_key, node1, EdgeType::Hyperlink, None)
                .is_none()
        );
        assert!(
            graph
                .add_edge(node1, invalid_key, EdgeType::Hyperlink, None)
                .is_none()
        );
    }

    #[test]
    fn test_add_multiple_edges() {
        let mut graph = Graph::new();
        let node1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let node2 = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));
        let node3 = graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 2.0));

        graph
            .add_edge(node1, node2, EdgeType::Hyperlink, None)
            .unwrap();
        graph
            .add_edge(node1, node3, EdgeType::Hyperlink, None)
            .unwrap();
        graph
            .add_edge(node2, node3, EdgeType::Hyperlink, None)
            .unwrap();

        assert_eq!(graph.edge_count(), 3);

        // Check node1 has 2 outgoing neighbors
        assert_eq!(graph.out_neighbors(node1).count(), 2);

        // Check node3 has 2 incoming neighbors
        assert_eq!(graph.in_neighbors(node3).count(), 2);
    }

    #[test]
    fn test_remove_edges_by_type_between_nodes() {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));

        graph.add_edge(a, b, EdgeType::Hyperlink, None).unwrap();
        graph.add_edge(a, b, EdgeType::UserGrouped, None).unwrap();

        let removed = graph.remove_edges(a, b, EdgeType::UserGrouped);
        assert_eq!(removed, 1);
        assert_eq!(graph.edge_count(), 1);
        let edge_key = graph.find_edge_key(a, b).expect("remaining hyperlink edge");
        let payload = graph.get_edge(edge_key).expect("remaining edge payload");
        assert!(payload.has_relation(RelationSelector::Semantic(SemanticSubKind::Hyperlink)));
        assert!(!payload.has_relation(RelationSelector::Semantic(SemanticSubKind::UserGrouped)));
    }

    #[test]
    fn test_add_edge_merges_semantics_on_single_stored_edge() {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));

        graph.add_edge(a, b, EdgeType::Hyperlink, None).unwrap();
        graph
            .add_edge(a, b, EdgeType::UserGrouped, Some("tab-group".to_string()))
            .unwrap();

        assert_eq!(graph.edge_count(), 1);
        let edge_key = graph.find_edge_key(a, b).unwrap();
        let payload = graph.get_edge(edge_key).unwrap();
        assert!(payload.has_edge_kind(EdgeType::Hyperlink));
        assert!(payload.has_edge_kind(EdgeType::UserGrouped));
        assert_eq!(payload.label(), Some("tab-group"));
        assert_eq!(graph.edges().count(), 2);
    }

    #[test]
    fn test_remove_node() {
        let mut graph = Graph::new();
        let n1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let n2 = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));
        graph.add_edge(n1, n2, EdgeType::Hyperlink, None);

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        assert!(graph.remove_node(n1));
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0); // edge auto-removed
        assert!(graph.get_node(n1).is_none());
        assert!(graph.get_node_by_url("https://a.com").is_none());

        // n2 still exists
        assert!(graph.get_node(n2).is_some());
    }

    #[test]
    fn test_remove_nonexistent_node() {
        let mut graph = Graph::new();
        assert!(!graph.remove_node(NodeIndex::new(999)));
    }

    #[test]
    fn test_nodes_iterator() {
        let mut graph = Graph::new();
        graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));
        graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 2.0));

        let urls: Vec<String> = graph.nodes().map(|(_, n)| n.url.clone()).collect();
        assert_eq!(urls.len(), 3);
        assert!(urls.contains(&"https://a.com".to_string()));
        assert!(urls.contains(&"https://b.com".to_string()));
        assert!(urls.contains(&"https://c.com".to_string()));
    }

    #[test]
    fn test_edges_iterator() {
        let mut graph = Graph::new();
        let node1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let node2 = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));
        let node3 = graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 2.0));

        graph.add_edge(node1, node2, EdgeType::Hyperlink, None);
        graph.add_edge(node1, node3, EdgeType::Hyperlink, None);

        let edge_count = graph.edges().count();
        assert_eq!(edge_count, 2);

        assert!(graph.inner.edge_references().all(|edge| {
            edge.weight()
                .has_relation(RelationSelector::Semantic(SemanticSubKind::Hyperlink))
        }));
    }

    #[test]
    fn test_node_lifecycle_default() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        let node = graph.get_node(key).unwrap();
        assert_eq!(node.lifecycle, NodeLifecycle::Cold);
    }

    #[test]
    fn test_empty_graph_operations() {
        let graph = Graph::new();

        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.get_node_by_url("https://example.com").is_none());

        let invalid_key = NodeIndex::new(999);
        assert!(graph.get_node(invalid_key).is_none());
    }

    #[test]
    fn test_node_count() {
        let mut graph = Graph::new();
        assert_eq!(graph.node_count(), 0);

        graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        assert_eq!(graph.node_count(), 1);

        graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn test_edge_count() {
        let mut graph = Graph::new();
        let node1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let node2 = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 1.0));

        assert_eq!(graph.edge_count(), 0);

        graph.add_edge(node1, node2, EdgeType::Hyperlink, None);
        assert_eq!(graph.edge_count(), 1);

        graph.add_edge(node2, node1, EdgeType::Hyperlink, None);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let mut graph = Graph::new();
        let n1 = graph.add_node("https://a.com".to_string(), Point2D::new(10.0, 20.0));
        let n2 = graph.add_node("https://b.com".to_string(), Point2D::new(30.0, 40.0));
        graph.add_edge(n1, n2, EdgeType::Hyperlink, None);

        graph.get_node_mut(n1).unwrap().title = "Site A".to_string();
        graph.get_node_mut(n2).unwrap().is_pinned = true;

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);

        assert_eq!(restored.node_count(), 2);
        assert_eq!(restored.edge_count(), 1);

        let (_, ra) = restored.get_node_by_url("https://a.com").unwrap();
        assert_eq!(ra.title, "Site A");
        assert_eq!(ra.position.x, 10.0);
        assert_eq!(ra.position.y, 20.0);

        let (_, rb) = restored.get_node_by_url("https://b.com").unwrap();
        assert!(rb.is_pinned);
        assert_eq!(rb.position.x, 30.0);
    }

    #[test]
    fn test_snapshot_empty_graph() {
        let graph = Graph::new();
        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);

        assert_eq!(restored.node_count(), 0);
        assert_eq!(restored.edge_count(), 0);
    }

    #[test]
    fn test_snapshot_preserves_edge_types() {
        let mut graph = Graph::new();
        let n1 = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let n2 = graph.add_node("https://b.com".to_string(), Point2D::new(100.0, 0.0));
        let n3 = graph.add_node("https://c.com".to_string(), Point2D::new(200.0, 0.0));
        graph.add_edge(n1, n2, EdgeType::Hyperlink, None);
        graph.add_edge(n2, n1, EdgeType::History, None);
        graph.add_edge(n1, n3, EdgeType::UserGrouped, None);

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);

        assert_eq!(restored.edge_count(), 3);

        let has_hyperlink = restored
            .find_edge_key(n1, n2)
            .and_then(|edge_key| restored.get_edge(edge_key))
            .is_some_and(|payload| {
                payload.has_relation(RelationSelector::Semantic(SemanticSubKind::Hyperlink))
            });
        let has_history = restored
            .find_edge_key(n2, n1)
            .and_then(|edge_key| restored.get_edge(edge_key))
            .is_some_and(|payload| {
                payload.has_relation(RelationSelector::Family(EdgeFamily::Traversal))
            });
        let has_user_grouped = restored
            .find_edge_key(n1, n3)
            .and_then(|edge_key| restored.get_edge(edge_key))
            .is_some_and(|payload| {
                payload.has_relation(RelationSelector::Semantic(SemanticSubKind::UserGrouped))
            });
        assert!(has_hyperlink);
        assert!(has_history);
        assert!(has_user_grouped);
    }

    #[test]
    fn test_snapshot_preserves_user_grouped_edge_label() {
        let mut graph = Graph::new();
        let from = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let to = graph.add_node("https://b.com".to_string(), Point2D::new(100.0, 0.0));
        graph
            .add_edge(
                from,
                to,
                EdgeType::UserGrouped,
                Some("tab-group".to_string()),
            )
            .unwrap();

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);
        let edge_key = restored.find_edge_key(from, to).unwrap();
        let payload = restored.get_edge(edge_key).unwrap();
        assert_eq!(payload.label(), Some("tab-group"));
    }

    #[test]
    fn test_snapshot_preserves_favicon_data() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let favicon = vec![255, 0, 0, 255];
        if let Some(node) = graph.get_node_mut(key) {
            node.favicon_rgba = Some(favicon.clone());
            node.favicon_width = 1;
            node.favicon_height = 1;
        }

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);
        let (_, restored_node) = restored.get_node_by_url("https://a.com").unwrap();
        assert_eq!(restored_node.favicon_rgba.as_ref(), Some(&favicon));
        assert_eq!(restored_node.favicon_width, 1);
        assert_eq!(restored_node.favicon_height, 1);
    }

    #[test]
    fn test_snapshot_preserves_thumbnail_data() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let thumbnail = vec![137, 80, 78, 71];
        if let Some(node) = graph.get_node_mut(key) {
            node.thumbnail_png = Some(thumbnail.clone());
            node.thumbnail_width = 64;
            node.thumbnail_height = 48;
        }

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);
        let (_, restored_node) = restored.get_node_by_url("https://a.com").unwrap();
        assert_eq!(restored_node.thumbnail_png.as_ref(), Some(&thumbnail));
        assert_eq!(restored_node.thumbnail_width, 64);
        assert_eq!(restored_node.thumbnail_height, 48);
    }

    #[test]
    fn test_snapshot_preserves_uuid_identity() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let node_id = graph.get_node(key).unwrap().id;

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);
        let (_, restored_node) = restored.get_node_by_id(node_id).unwrap();
        assert_eq!(restored_node.url, "https://a.com");
    }

    // --- TEST-3: from_snapshot edge cases ---

    #[test]
    fn test_snapshot_edge_with_missing_url_is_dropped() {
        use crate::services::persistence::types::{
            GraphSnapshot, PersistedAddressKind, PersistedEdge, PersistedNode,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: Uuid::new_v4().to_string(),
                url: "https://a.com".to_string(),
                cached_host: None,
                title: String::new(),
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
                address_kind: PersistedAddressKind::Http,
                classifications: Vec::new(),
                frame_layout_hints: Vec::new(),
                frame_split_offer_suppressed: false,
            }],
            edges: vec![PersistedEdge {
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
            }],
            import_records: vec![],
            timestamp_secs: 0,
        };

        let graph = Graph::from_snapshot(&snapshot);

        // Node should be restored, edge should be silently dropped
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_snapshot_duplicate_urls_last_wins() {
        use crate::services::persistence::types::{
            GraphSnapshot, PersistedAddressKind, PersistedNode,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![
                PersistedNode {
                    node_id: Uuid::new_v4().to_string(),
                    url: "https://same.com".to_string(),
                    cached_host: None,
                    title: "First".to_string(),
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
                    address_kind: PersistedAddressKind::Http,
                    classifications: Vec::new(),
                    frame_layout_hints: Vec::new(),
                    frame_split_offer_suppressed: false,
                },
                PersistedNode {
                    node_id: Uuid::new_v4().to_string(),
                    url: "https://same.com".to_string(),
                    cached_host: None,
                    title: "Second".to_string(),
                    position_x: 100.0,
                    position_y: 100.0,
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
                    address_kind: PersistedAddressKind::Http,
                    classifications: Vec::new(),
                    frame_layout_hints: Vec::new(),
                    frame_split_offer_suppressed: false,
                },
            ],
            edges: vec![],
            import_records: vec![],
            timestamp_secs: 0,
        };

        let graph = Graph::from_snapshot(&snapshot);

        // Both nodes are created and lookup keeps last inserted semantics.
        assert_eq!(graph.node_count(), 2);
        let (_, node) = graph.get_node_by_url("https://same.com").unwrap();
        assert_eq!(node.title, "Second");
    }

    #[test]
    fn test_update_node_url() {
        let mut graph = Graph::new();
        let key = graph.add_node("old".to_string(), Point2D::new(0.0, 0.0));

        let old = graph.update_node_url(key, "new".to_string());

        assert_eq!(old, Some("old".to_string()));
        assert_eq!(graph.get_node(key).unwrap().url, "new");
        assert!(graph.get_node_by_url("new").is_some());
        assert!(graph.get_node_by_url("old").is_none());
    }

    #[test]
    fn test_update_node_url_nonexistent() {
        let mut graph = Graph::new();
        let fake_key = NodeKey::new(999);

        assert_eq!(graph.update_node_url(fake_key, "x".to_string()), None);
    }

    #[test]
    fn test_cold_restore_reapplies_history_index() {
        use crate::services::persistence::types::{
            GraphSnapshot, PersistedAddressKind, PersistedNode, PersistedNodeSessionState,
        };

        let node_id = Uuid::new_v4();
        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: node_id.to_string(),
                url: "https://fallback.example".to_string(),
                cached_host: None,
                title: "Node".to_string(),
                position_x: 0.0,
                position_y: 0.0,
                tags: vec![],
                tag_presentation: NodeTagPresentationState::default(),
                import_provenance: vec![],
                is_pinned: false,
                history_entries: vec!["https://legacy.example".to_string()],
                history_index: 0,
                thumbnail_png: None,
                thumbnail_width: 0,
                thumbnail_height: 0,
                favicon_rgba: None,
                favicon_width: 0,
                favicon_height: 0,
                session_state: Some(PersistedNodeSessionState {
                    history_entries: vec![
                        "https://example.com/one".to_string(),
                        "https://example.com/two".to_string(),
                        "https://example.com/three".to_string(),
                    ],
                    history_index: 2,
                    scroll_x: Some(4.0),
                    scroll_y: Some(120.0),
                    form_draft: None,
                }),
                mime_hint: None,
                address_kind: PersistedAddressKind::Http,
                classifications: Vec::new(),
                frame_layout_hints: Vec::new(),
                frame_split_offer_suppressed: false,
            }],
            edges: vec![],
            import_records: vec![],
            timestamp_secs: 0,
        };

        let restored = Graph::from_snapshot(&snapshot);
        let (_, node) = restored.get_node_by_id(node_id).unwrap();
        assert_eq!(node.history_entries.len(), 3);
        assert_eq!(node.history_index, 2);
    }

    #[test]
    fn test_cold_restore_reapplies_scroll_offset() {
        use crate::services::persistence::types::{
            GraphSnapshot, PersistedAddressKind, PersistedNode, PersistedNodeSessionState,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: Uuid::new_v4().to_string(),
                url: "https://example.com".to_string(),
                cached_host: None,
                title: "Node".to_string(),
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
                session_state: Some(PersistedNodeSessionState {
                    history_entries: vec!["https://example.com".to_string()],
                    history_index: 0,
                    scroll_x: Some(20.0),
                    scroll_y: Some(640.0),
                    form_draft: None,
                }),
                mime_hint: None,
                address_kind: PersistedAddressKind::Http,
                classifications: Vec::new(),
                frame_layout_hints: Vec::new(),
                frame_split_offer_suppressed: false,
            }],
            edges: vec![],
            import_records: vec![],
            timestamp_secs: 0,
        };

        let restored = Graph::from_snapshot(&snapshot);
        let (_, node) = restored.get_node_by_url("https://example.com").unwrap();
        assert_eq!(node.session_scroll, Some((20.0, 640.0)));
    }

    #[test]
    fn test_restore_fallback_without_session_state() {
        use crate::services::persistence::types::{
            GraphSnapshot, PersistedAddressKind, PersistedNode,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: Uuid::new_v4().to_string(),
                url: "https://fallback.example".to_string(),
                cached_host: None,
                title: "Node".to_string(),
                position_x: 0.0,
                position_y: 0.0,
                tags: vec![],
                tag_presentation: NodeTagPresentationState::default(),
                import_provenance: vec![],
                is_pinned: false,
                history_entries: vec!["https://legacy-one.example".to_string()],
                history_index: 0,
                thumbnail_png: None,
                thumbnail_width: 0,
                thumbnail_height: 0,
                favicon_rgba: None,
                favicon_width: 0,
                favicon_height: 0,
                session_state: None,
                mime_hint: None,
                address_kind: PersistedAddressKind::Http,
                classifications: Vec::new(),
                frame_layout_hints: Vec::new(),
                frame_split_offer_suppressed: false,
            }],
            edges: vec![],
            import_records: vec![],
            timestamp_secs: 0,
        };

        let restored = Graph::from_snapshot(&snapshot);
        let (_, node) = restored
            .get_node_by_url("https://fallback.example")
            .unwrap();
        assert_eq!(
            node.history_entries,
            vec!["https://legacy-one.example".to_string()]
        );
        assert_eq!(node.history_index, 0);
        assert_eq!(node.session_scroll, None);
    }

    #[test]
    fn test_snapshot_roundtrip_preserves_import_provenance() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        assert!(graph.set_node_import_provenance(
            key,
            vec![NodeImportProvenance {
                source_id: "import:firefox-bookmarks".to_string(),
                source_label: "Firefox bookmarks".to_string(),
            }],
        ));

        let restored = Graph::from_snapshot(&graph.to_snapshot());
        let (_, node) = restored.get_node_by_url("https://example.com").unwrap();
        assert_eq!(
            node.import_provenance,
            vec![NodeImportProvenance {
                source_id: "import:firefox-bookmarks".to_string(),
                source_label: "Firefox bookmarks".to_string(),
            }]
        );
    }

    #[test]
    fn test_snapshot_roundtrip_preserves_classifications() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        let classification = NodeClassification {
            scheme: ClassificationScheme::Udc,
            value: "udc:519.6".to_string(),
            label: Some("Computational mathematics".to_string()),
            confidence: 0.9,
            provenance: ClassificationProvenance::UserAuthored,
            status: ClassificationStatus::Accepted,
            primary: true,
        };
        assert!(graph.add_node_classification(key, classification.clone()));

        let restored = Graph::from_snapshot(&graph.to_snapshot());
        let (_, node) = restored.get_node_by_url("https://example.com").unwrap();
        assert_eq!(node.classifications.len(), 1);
        let c = &node.classifications[0];
        assert_eq!(c.scheme, ClassificationScheme::Udc);
        assert_eq!(c.value, "udc:519.6");
        assert_eq!(c.label.as_deref(), Some("Computational mathematics"));
        assert!((c.confidence - 0.9).abs() < 1e-6);
        assert_eq!(c.provenance, ClassificationProvenance::UserAuthored);
        assert_eq!(c.status, ClassificationStatus::Accepted);
        assert!(c.primary);
    }

    #[test]
    fn test_snapshot_roundtrip_preserves_imported_and_provenance_edge_sub_kinds() {
        let mut graph = Graph::new();
        let from = graph.add_node("https://from.example".to_string(), Point2D::new(0.0, 0.0));
        let to = graph.add_node("https://to.example".to_string(), Point2D::new(10.0, 0.0));

        let _ = graph.assert_relation(
            from,
            to,
            EdgeAssertion::Imported {
                sub_kind: ImportedSubKind::BookmarkFolder,
            },
        );
        let _ = graph.assert_relation(
            from,
            to,
            EdgeAssertion::Provenance {
                sub_kind: ProvenanceSubKind::ClippedFrom,
            },
        );

        let restored = Graph::from_snapshot(&graph.to_snapshot());
        let edge_key = restored
            .find_edge_key(from, to)
            .expect("restored imported/provenance edge");
        let payload = restored.get_edge(edge_key).expect("restored payload");

        assert!(payload.has_relation(RelationSelector::Imported(ImportedSubKind::BookmarkFolder)));
        assert!(payload.has_relation(RelationSelector::Provenance(ProvenanceSubKind::ClippedFrom)));
    }

    #[test]
    fn test_snapshot_roundtrip_preserves_import_records_and_suppression_state() {
        let mut graph = Graph::new();
        let included = graph.add_node(
            "https://included.example".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let suppressed = graph.add_node(
            "https://suppressed.example".to_string(),
            Point2D::new(10.0, 0.0),
        );
        let included_id = graph
            .get_node(included)
            .expect("included node")
            .id
            .to_string();
        let suppressed_id = graph
            .get_node(suppressed)
            .expect("suppressed node")
            .id
            .to_string();
        assert!(graph.set_import_records(vec![ImportRecord {
            record_id: "import-record:firefox-bookmarks-2026-03-17".to_string(),
            source_id: "import:firefox-bookmarks".to_string(),
            source_label: "Firefox bookmarks".to_string(),
            imported_at_secs: 1_763_500_800,
            memberships: vec![
                ImportRecordMembership {
                    node_id: included_id,
                    suppressed: false,
                },
                ImportRecordMembership {
                    node_id: suppressed_id,
                    suppressed: true,
                },
            ],
        }]));

        let restored = Graph::from_snapshot(&graph.to_snapshot());
        let restored_records = restored.import_records();
        assert_eq!(restored_records.len(), 1);
        assert_eq!(
            restored_records[0].record_id,
            "import-record:firefox-bookmarks-2026-03-17"
        );
        assert_eq!(restored_records[0].memberships.len(), 2);
        assert!(
            restored_records[0]
                .memberships
                .iter()
                .any(|membership| membership.suppressed)
        );
        assert_eq!(
            restored
                .node_import_provenance(included)
                .expect("active provenance should exist")
                .len(),
            1
        );
        assert!(
            restored
                .node_import_provenance(suppressed)
                .expect("suppressed provenance should resolve to empty slice")
                .is_empty()
        );
    }

    #[test]
    fn test_delete_import_record_removes_derived_provenance() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let node_id = graph.get_node(key).expect("node").id.to_string();
        assert!(graph.set_import_records(vec![ImportRecord {
            record_id: "import-record:test".to_string(),
            source_id: "import:test".to_string(),
            source_label: "Test import".to_string(),
            imported_at_secs: 1_763_500_800,
            memberships: vec![ImportRecordMembership {
                node_id,
                suppressed: false,
            }],
        }]));

        assert!(graph.delete_import_record("import-record:test"));
        assert!(graph.import_records().is_empty());
        assert!(
            graph
                .node_import_provenance(key)
                .expect("node provenance slice")
                .is_empty()
        );
    }

    #[test]
    fn test_suppress_import_record_membership_updates_node_projection() {
        let mut graph = Graph::new();
        let active = graph.add_node("https://active.example".to_string(), Point2D::new(0.0, 0.0));
        let peer = graph.add_node("https://peer.example".to_string(), Point2D::new(10.0, 0.0));
        let active_id = graph.get_node(active).expect("active").id.to_string();
        let peer_id = graph.get_node(peer).expect("peer").id.to_string();
        assert!(graph.set_import_records(vec![ImportRecord {
            record_id: "import-record:test".to_string(),
            source_id: "import:test".to_string(),
            source_label: "Test import".to_string(),
            imported_at_secs: 1_763_500_800,
            memberships: vec![
                ImportRecordMembership {
                    node_id: active_id,
                    suppressed: false,
                },
                ImportRecordMembership {
                    node_id: peer_id,
                    suppressed: false,
                },
            ],
        }]));

        assert!(graph.set_import_record_membership_suppressed("import-record:test", active, true,));
        assert!(
            graph
                .node_import_provenance(active)
                .expect("active provenance slice")
                .is_empty()
        );
        assert_eq!(
            graph.import_record_member_keys("import-record:test"),
            vec![peer]
        );
    }

    // --- MIME / address-kind detection tests ---

    #[test]
    fn node_created_with_http_url_has_http_address_kind() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let node = graph.get_node(key).unwrap();
        assert_eq!(node.address_kind, AddressKind::Http);
    }

    #[test]
    fn node_created_with_file_url_has_file_address_kind() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "file:///home/user/doc.pdf".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node = graph.get_node(key).unwrap();
        assert_eq!(node.address_kind, AddressKind::File);
    }

    #[test]
    fn node_created_with_file_pdf_url_gets_pdf_mime_hint() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "file:///home/user/document.pdf".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node = graph.get_node(key).unwrap();
        assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
        assert_eq!(node.address_kind, AddressKind::File);
    }

    #[test]
    fn node_created_with_http_url_has_no_mime_hint_by_default() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "https://example.com/page".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node = graph.get_node(key).unwrap();
        // Plain HTTP URLs without a recognisable extension yield no MIME hint.
        assert!(node.mime_hint.is_none());
    }

    #[test]
    fn detect_mime_returns_pdf_for_pdf_path() {
        assert_eq!(
            detect_mime("file:///home/user/document.pdf", None),
            Some("application/pdf".to_string())
        );
    }

    #[test]
    fn detect_mime_returns_text_plain_for_txt_path() {
        assert_eq!(
            detect_mime("file:///notes/readme.txt", None),
            Some("text/plain".to_string())
        );
    }

    #[test]
    fn detect_mime_returns_none_for_no_extension() {
        assert!(detect_mime("https://example.com/page", None).is_none());
    }

    #[test]
    fn detect_mime_uses_magic_bytes_when_extension_is_missing() {
        let pdf_header = b"%PDF-1.7\n1 0 obj\n";
        assert_eq!(
            detect_mime("https://example.com/no-extension", Some(pdf_header)),
            Some("application/pdf".to_string())
        );
    }

    #[test]
    fn detect_mime_prefers_extension_when_unambiguous() {
        let pdf_header = b"%PDF-1.7\n1 0 obj\n";
        assert_eq!(
            detect_mime("file:///home/user/readme.txt", Some(pdf_header)),
            Some("text/plain".to_string())
        );
    }

    #[test]
    fn detect_mime_falls_back_to_extension_when_magic_unknown() {
        let unknown = b"not a known magic signature";
        assert_eq!(
            detect_mime("file:///home/user/data.json", Some(unknown)),
            Some("application/json".to_string())
        );
    }

    #[test]
    fn address_kind_from_url_http() {
        assert_eq!(
            address_kind_from_url("http://example.com"),
            AddressKind::Http
        );
        assert_eq!(
            address_kind_from_url("https://example.com"),
            AddressKind::Http
        );
    }

    #[test]
    fn address_kind_from_url_file() {
        assert_eq!(
            address_kind_from_url("file:///home/user/file.txt"),
            AddressKind::File
        );
    }

    #[test]
    fn address_kind_from_url_data_clip_directory_and_unknown() {
        let temp_dir = tempfile::tempdir().expect("temp dir should build");
        let dir_url = url::Url::from_directory_path(temp_dir.path())
            .expect("directory URL should build")
            .to_string();

        assert_eq!(
            address_kind_from_url("data:text/plain,hello"),
            AddressKind::Data
        );
        assert_eq!(
            address_kind_from_url("verso://clip/clip-123"),
            AddressKind::GraphshellClip
        );
        assert_eq!(address_kind_from_url(&dir_url), AddressKind::Directory);
        assert_eq!(
            address_kind_from_url("gemini://gemini.circumlunar.space/"),
            AddressKind::Unknown
        );
        assert_eq!(
            address_kind_from_url("ftp://files.example.com/"),
            AddressKind::Unknown
        );
    }

    #[test]
    fn address_kind_from_url_file_directory_classification_is_syntax_based() {
        assert_eq!(
            address_kind_from_url("file:///tmp/sample-dir/"),
            AddressKind::Directory
        );
        assert_eq!(
            address_kind_from_url("file:///tmp/sample-dir"),
            AddressKind::File
        );
    }

    #[test]
    fn snapshot_roundtrip_preserves_mime_hint_and_address_kind() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "file:///home/user/report.pdf".to_string(),
            Point2D::new(0.0, 0.0),
        );
        assert_eq!(
            graph.get_node(key).unwrap().mime_hint.as_deref(),
            Some("application/pdf")
        );
        assert_eq!(graph.get_node(key).unwrap().address_kind, AddressKind::File);

        let snapshot = graph.to_snapshot();
        let restored = Graph::from_snapshot(&snapshot);
        let (_, rnode) = restored
            .get_node_by_url("file:///home/user/report.pdf")
            .unwrap();
        assert_eq!(rnode.mime_hint.as_deref(), Some("application/pdf"));
        assert_eq!(rnode.address_kind, AddressKind::File);
    }

    #[test]
    fn hop_distances_shortest_path_and_reachability_use_undirected_connectivity() {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 0.0));
        let c = graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 0.0));
        let d = graph.add_node("https://d.com".to_string(), Point2D::new(3.0, 0.0));

        let _ = graph.add_edge(a, b, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(b, c, EdgeType::Hyperlink, None);

        let hops = graph.hop_distances_from(a);
        assert_eq!(hops.get(&a).copied(), Some(0));
        assert_eq!(hops.get(&b).copied(), Some(1));
        assert_eq!(hops.get(&c).copied(), Some(2));
        assert!(hops.get(&d).is_none());

        let path = graph.shortest_path(a, c).expect("path should exist");
        assert_eq!(path.first().copied(), Some(a));
        assert_eq!(path.last().copied(), Some(c));

        assert!(graph.is_reachable(a, c));
        assert!(!graph.is_reachable(a, d));
    }

    #[test]
    fn orphan_and_weak_component_accessors_report_expected_partitions() {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 0.0));
        let c = graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 0.0));
        let d = graph.add_node("https://d.com".to_string(), Point2D::new(3.0, 0.0));
        let e = graph.add_node("https://e.com".to_string(), Point2D::new(4.0, 0.0));

        let _ = graph.add_edge(a, b, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(d, e, EdgeType::Hyperlink, None);

        let mut orphans = graph.orphan_node_keys();
        orphans.sort_by_key(|k| k.index());
        assert_eq!(orphans, vec![c]);

        let mut sizes: Vec<usize> = graph
            .weakly_connected_components()
            .into_iter()
            .map(|component| component.len())
            .collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![1, 2, 2]);
    }

    #[test]
    fn component_accessors_handle_empty_graph() {
        let graph = Graph::new();

        assert!(graph.orphan_node_keys().is_empty());
        assert!(graph.weakly_connected_components().is_empty());
        assert!(graph.strongly_connected_components().is_empty());
    }

    #[test]
    fn sorted_neighbor_and_connected_import_accessors_are_stable() {
        let mut graph = Graph::new();
        let seed = graph.add_node("https://seed.example".to_string(), Point2D::new(0.0, 0.0));
        let left = graph.add_node("https://left.example".to_string(), Point2D::new(1.0, 0.0));
        let right = graph.add_node("https://right.example".to_string(), Point2D::new(2.0, 0.0));
        let shared = graph.add_node("https://shared.example".to_string(), Point2D::new(2.5, 0.0));
        let isolated = graph.add_node(
            "https://isolated.example".to_string(),
            Point2D::new(3.0, 0.0),
        );

        let _ = graph.add_edge(seed, right, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(left, seed, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(left, shared, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(right, shared, EdgeType::Hyperlink, None);

        let sorted_neighbors = graph.neighbors_undirected_sorted(seed);
        assert_eq!(sorted_neighbors, vec![left, right]);

        let import_nodes = graph.connected_frame_import_nodes(&[isolated, seed]);
        assert_eq!(import_nodes, vec![seed, left, right, isolated]);

        let depth_one = graph.connected_candidates_with_depth(seed, 1);
        assert_eq!(depth_one, vec![(left, 1), (right, 1)]);

        let depth_two = graph.connected_candidates_with_depth(seed, 2);
        assert!(depth_two.contains(&(left, 1)));
        assert!(depth_two.contains(&(right, 1)));
        assert!(depth_two.contains(&(shared, 2)));
    }

    #[test]
    fn strongly_connected_components_reports_cycle_partition() {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.com".to_string(), Point2D::new(1.0, 0.0));
        let c = graph.add_node("https://c.com".to_string(), Point2D::new(2.0, 0.0));
        let d = graph.add_node("https://d.com".to_string(), Point2D::new(3.0, 0.0));

        let _ = graph.add_edge(a, b, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(b, c, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(c, a, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(c, d, EdgeType::Hyperlink, None);

        let mut sizes: Vec<usize> = graph
            .strongly_connected_components()
            .into_iter()
            .map(|component| component.len())
            .collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![1, 3]);
    }

    #[test]
    fn removing_tag_prunes_stale_icon_override() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        assert!(graph.insert_node_tag(key, "research".to_string()));
        assert!(graph.set_node_tag_icon_override(
            key,
            "research",
            Some(crate::graph::badge::BadgeIcon::Emoji("🔬".to_string()))
        ));

        assert!(graph.remove_node_tag(key, "research"));
        assert!(
            graph
                .node_tag_presentation(key)
                .is_some_and(|presentation| presentation.icon_overrides.is_empty())
        );
    }

    #[test]
    fn system_tag_icon_cannot_be_overridden() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        assert!(graph.insert_node_tag(key, "#pin".to_string()));
        assert!(!graph.set_node_tag_icon_override(
            key,
            "#pin",
            Some(crate::graph::badge::BadgeIcon::Emoji("🔬".to_string()))
        ));
    }

    #[test]
    fn frame_layout_metadata_survives_snapshot_roundtrip() {
        let mut graph = Graph::new();
        let frame = graph.add_node("verso://frame/demo".to_string(), Point2D::new(0.0, 0.0));
        let first = graph.add_node("https://first.example".to_string(), Point2D::new(1.0, 0.0));
        let second = graph.add_node("https://second.example".to_string(), Point2D::new(2.0, 0.0));
        let first_id = graph.get_node(first).unwrap().id.to_string();
        let second_id = graph.get_node(second).unwrap().id.to_string();

        assert!(graph.append_frame_layout_hint(
            frame,
            FrameLayoutHint::SplitHalf {
                first: first_id.clone(),
                second: second_id.clone(),
                orientation: SplitOrientation::Horizontal,
            },
        ));
        assert!(graph.set_frame_split_offer_suppressed(frame, true));

        let restored = Graph::from_snapshot(&graph.to_snapshot());
        let (restored_frame, _) = restored.get_node_by_url("verso://frame/demo").unwrap();
        let hints = restored.frame_layout_hints(restored_frame).unwrap();

        assert_eq!(hints.len(), 1);
        assert_eq!(
            hints[0],
            FrameLayoutHint::SplitHalf {
                first: first_id,
                second: second_id,
                orientation: SplitOrientation::Horizontal,
            }
        );
        assert_eq!(
            restored.frame_split_offer_suppressed(restored_frame),
            Some(true)
        );
    }

    #[test]
    fn frame_layout_hint_move_reorders_hints() {
        let mut graph = Graph::new();
        let frame = graph.add_node("verso://frame/demo".to_string(), Point2D::new(0.0, 0.0));
        let first = graph.add_node("https://first.example".to_string(), Point2D::new(1.0, 0.0));
        let second = graph.add_node("https://second.example".to_string(), Point2D::new(2.0, 0.0));
        let third = graph.add_node("https://third.example".to_string(), Point2D::new(3.0, 0.0));
        let first_id = graph.get_node(first).unwrap().id.to_string();
        let second_id = graph.get_node(second).unwrap().id.to_string();
        let third_id = graph.get_node(third).unwrap().id.to_string();

        assert!(graph.append_frame_layout_hint(
            frame,
            FrameLayoutHint::SplitHalf {
                first: first_id.clone(),
                second: second_id.clone(),
                orientation: SplitOrientation::Vertical,
            },
        ));
        assert!(graph.append_frame_layout_hint(
            frame,
            FrameLayoutHint::SplitHalf {
                first: second_id.clone(),
                second: third_id.clone(),
                orientation: SplitOrientation::Horizontal,
            },
        ));

        assert!(graph.move_frame_layout_hint(frame, 1, 0));
        let hints = graph.frame_layout_hints(frame).unwrap();
        assert_eq!(
            hints[0],
            FrameLayoutHint::SplitHalf {
                first: second_id.clone(),
                second: third_id,
                orientation: SplitOrientation::Horizontal,
            }
        );
        assert_eq!(
            hints[1],
            FrameLayoutHint::SplitHalf {
                first: first_id,
                second: second_id.clone(),
                orientation: SplitOrientation::Vertical,
            }
        );
        assert!(!graph.move_frame_layout_hint(frame, 1, 1));
    }
}
