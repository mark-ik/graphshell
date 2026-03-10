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
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use petgraph::visit::UndirectedAdaptor;
use petgraph::{Directed, Direction};
use rkyv::{
    Archive, Archived, Deserialize, Place, Resolver, Serialize,
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::services::persistence::types::{
    GraphSnapshot, PersistedAddressKind, PersistedEdge, PersistedEdgeType, PersistedNode,
    PersistedNodeSessionState,
};

pub mod apply;
pub mod egui_adapter;

/// Stable node handle (petgraph NodeIndex — survives other deletions)
pub type NodeKey = NodeIndex;

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
    fn deserialize_with(field: &Archived<[u8; 16]>, deserializer: &mut D) -> Result<Uuid, D::Error> {
        let bytes = field.deserialize(deserializer)?;
        Ok(Uuid::from_bytes(bytes))
    }
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
    /// Any other scheme; renderer selected by `ViewerRegistry`.
    Custom,
}

/// Infer `AddressKind` from a URL scheme.
pub(crate) fn address_kind_from_url(url: &str) -> AddressKind {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("file://") {
        AddressKind::File
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        AddressKind::Http
    } else {
        AddressKind::Custom
    }
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
            lifecycle: NodeLifecycle::Cold,
        }
    }
}

/// Type of edge connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum EdgeType {
    /// Hyperlink from one page to another
    Hyperlink,

    /// Browser history traversal
    History,

    /// Explicit user grouping association
    UserGrouped,
}

/// Canonical edge kind set entry.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Archive, Serialize, Deserialize,
)]
#[rkyv(compare(PartialEq, PartialOrd), derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum EdgeKind {
    Hyperlink,
    TraversalDerived,
    UserGrouped,
    AgentDerived,
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

/// Edge semantics payload: structural assertions + temporal traversal events.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct EdgePayload {
    pub kinds: BTreeSet<EdgeKind>,
    pub user_grouped: Option<UserGroupedData>,
    pub traversal: Option<TraversalData>,
}

impl EdgePayload {
    pub fn new() -> Self {
        Self {
            kinds: BTreeSet::new(),
            user_grouped: None,
            traversal: None,
        }
    }

    pub fn from_edge_type(edge_type: EdgeType, label: Option<String>) -> Self {
        let mut payload = Self::new();
        let _ = payload.add_edge_kind(edge_type, label);
        payload
    }

    pub fn add_edge_kind(&mut self, edge_type: EdgeType, label: Option<String>) -> bool {
        match edge_type {
            EdgeType::Hyperlink => self.kinds.insert(EdgeKind::Hyperlink),
            EdgeType::UserGrouped => {
                let inserted = self.kinds.insert(EdgeKind::UserGrouped);
                let data = self.user_grouped.get_or_insert_with(UserGroupedData::default);
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
                let had_data = self.traversal.is_some();
                let _ = self.traversal.get_or_insert_with(TraversalData::default);
                inserted || !had_data
            }
        }
    }

    pub fn add_edge_type(&mut self, edge_type: EdgeType) {
        let _ = self.add_edge_kind(edge_type, None);
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
        }
    }

    pub fn has_edge_type(&self, edge_type: EdgeType) -> bool {
        self.has_edge_kind(edge_type)
    }

    pub fn has_kind(&self, kind: EdgeKind) -> bool {
        self.kinds.contains(&kind)
    }

    pub fn remove_edge_kind(&mut self, edge_type: EdgeType) -> bool {
        match edge_type {
            EdgeType::Hyperlink if self.kinds.remove(&EdgeKind::Hyperlink) => true,
            EdgeType::UserGrouped if self.kinds.remove(&EdgeKind::UserGrouped) => {
                self.user_grouped = None;
                true
            }
            EdgeType::History if self.kinds.remove(&EdgeKind::TraversalDerived) => {
                self.traversal = None;
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
        self.traversal
            .get_or_insert_with(TraversalData::default)
            .push(traversal);
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

/// Main graph structure backed by petgraph::StableGraph
#[derive(Clone)]
pub struct Graph {
    /// The underlying petgraph stable graph
    pub(crate) inner: StableGraph<Node, EdgePayload, Directed>,

    /// URL to node mapping for lookup (supports duplicate URLs).
    url_to_nodes: HashMap<String, Vec<NodeKey>>,

    /// Stable UUID to node mapping.
    id_to_node: HashMap<Uuid, NodeKey>,
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            inner: StableGraph::new(),
            url_to_nodes: HashMap::new(),
            id_to_node: HashMap::new(),
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
            let mut out = Vec::with_capacity(3);
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
            out.into_iter()
        })
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
        dijkstra(
            &UndirectedAdaptor(&self.inner),
            source,
            None,
            |_| 1_usize,
        )
        .into_iter()
        .collect()
    }

    /// Nodes with no incoming or outgoing edges.
    pub fn orphan_node_keys(&self) -> Vec<NodeKey> {
        self.inner
            .node_indices()
            .filter(|&key| {
                self.inner.edges_directed(key, Direction::Outgoing).next().is_none()
                    && self.inner.edges_directed(key, Direction::Incoming).next().is_none()
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
                mime_hint: node.mime_hint.clone(),
                address_kind: match node.address_kind {
                    AddressKind::Http => PersistedAddressKind::Http,
                    AddressKind::File => PersistedAddressKind::File,
                    AddressKind::Custom => PersistedAddressKind::Custom,
                },
            })
            .collect();

        let edges = self
            .inner
            .edge_references()
            .flat_map(|edge| {
                let from_node_id = self
                    .get_node(edge.source())
                    .map(|n| n.id.to_string())
                    .unwrap_or_default();
                let to_node_id = self
                    .get_node(edge.target())
                    .map(|n| n.id.to_string())
                    .unwrap_or_default();
                let payload = edge.weight();
                let mut persisted_edges = Vec::with_capacity(3);
                if payload.has_kind(EdgeKind::Hyperlink) {
                    persisted_edges.push(PersistedEdge {
                        from_node_id: from_node_id.clone(),
                        to_node_id: to_node_id.clone(),
                        edge_type: PersistedEdgeType::Hyperlink,
                        edge_label: payload.label().map(str::to_string),
                    });
                }
                if payload.has_kind(EdgeKind::TraversalDerived) {
                    persisted_edges.push(PersistedEdge {
                        from_node_id: from_node_id.clone(),
                        to_node_id: to_node_id.clone(),
                        edge_type: PersistedEdgeType::History,
                        edge_label: payload.label().map(str::to_string),
                    });
                }
                if payload.has_kind(EdgeKind::UserGrouped) {
                    persisted_edges.push(PersistedEdge {
                        from_node_id,
                        to_node_id,
                        edge_type: PersistedEdgeType::UserGrouped,
                        edge_label: payload.label().map(str::to_string),
                    });
                }
                persisted_edges
            })
            .collect();

        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        GraphSnapshot {
            nodes,
            edges,
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
                    PersistedAddressKind::Custom => AddressKind::Custom,
                };
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
                let edge_type = match pedge.edge_type {
                    PersistedEdgeType::Hyperlink => EdgeType::Hyperlink,
                    PersistedEdgeType::History => EdgeType::History,
                    PersistedEdgeType::UserGrouped => EdgeType::UserGrouped,
                };
                let _ = graph.add_edge(from, to, edge_type, pedge.edge_label.clone());
            }
        }

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

        graph.add_edge(node1, node2, EdgeType::Hyperlink, None).unwrap();

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

        graph.add_edge(node1, node2, EdgeType::Hyperlink, None).unwrap();
        graph.add_edge(node1, node3, EdgeType::Hyperlink, None).unwrap();
        graph.add_edge(node2, node3, EdgeType::Hyperlink, None).unwrap();

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
        graph.add_edge(a, b, EdgeType::UserGrouped, None).unwrap();

        let removed = graph.remove_edges(a, b, EdgeType::UserGrouped);
        assert_eq!(removed, 1);
        assert_eq!(graph.edge_count(), 1);
        assert!(
            graph
                .edges()
                .all(|edge| edge.edge_type == EdgeType::Hyperlink)
        );
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

        let edge_types: Vec<EdgeType> = graph.edges().map(|e| e.edge_type).collect();
        assert!(edge_types.iter().all(|&t| t == EdgeType::Hyperlink));
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

        let edges: Vec<_> = restored.edges().collect();
        let has_hyperlink = edges.iter().any(|e| e.edge_type == EdgeType::Hyperlink);
        let has_history = edges.iter().any(|e| e.edge_type == EdgeType::History);
        let has_user_grouped = edges.iter().any(|e| e.edge_type == EdgeType::UserGrouped);
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
            GraphSnapshot, PersistedAddressKind, PersistedEdge, PersistedEdgeType, PersistedNode,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: Uuid::new_v4().to_string(),
                url: "https://a.com".to_string(),
                cached_host: None,
                title: String::new(),
                position_x: 0.0,
                position_y: 0.0,
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
            }],
            edges: vec![PersistedEdge {
                from_node_id: Uuid::new_v4().to_string(),
                to_node_id: Uuid::new_v4().to_string(),
                edge_type: PersistedEdgeType::Hyperlink,
                edge_label: None,
            }],
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
                },
                PersistedNode {
                    node_id: Uuid::new_v4().to_string(),
                    url: "https://same.com".to_string(),
                    cached_host: None,
                    title: "Second".to_string(),
                    position_x: 100.0,
                    position_y: 100.0,
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
                },
            ],
            edges: vec![],
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
            }],
            edges: vec![],
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
            }],
            edges: vec![],
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
            }],
            edges: vec![],
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
    fn address_kind_from_url_custom() {
        assert_eq!(
            address_kind_from_url("gemini://gemini.circumlunar.space/"),
            AddressKind::Custom
        );
        assert_eq!(
            address_kind_from_url("ftp://files.example.com/"),
            AddressKind::Custom
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
        let isolated =
            graph.add_node("https://isolated.example".to_string(), Point2D::new(3.0, 0.0));

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
}
