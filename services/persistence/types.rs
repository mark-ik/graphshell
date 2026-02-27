/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Serializable types for graph persistence.

use rkyv::{Archive, Deserialize, Serialize};

/// Address type hint for persistence (mirrors `AddressKind` in the graph model).
#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedAddressKind {
    Http,
    File,
    Custom,
}

impl Default for PersistedAddressKind {
    fn default() -> Self {
        Self::Http
    }
}

/// Persisted per-node session fidelity state.
#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct PersistedNodeSessionState {
    pub history_entries: Vec<String>,
    pub history_index: usize,
    pub scroll_x: Option<f32>,
    pub scroll_y: Option<f32>,
    pub form_draft: Option<String>,
}

/// Persisted node.
#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct PersistedNode {
    /// Stable node identity.
    pub node_id: String,
    pub url: String,
    pub title: String,
    pub position_x: f32,
    pub position_y: f32,
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
    /// Address type hint; inferred from URL scheme.
    pub address_kind: PersistedAddressKind,
}

/// Edge type for persistence.
#[derive(
    Archive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedEdgeType {
    Hyperlink,
    History,
    UserGrouped,
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
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq))]
pub enum PersistedNavigationTrigger {
    Unknown,
    Back,
    Forward,
}

/// Persisted edge.
#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct PersistedEdge {
    pub from_node_id: String,
    pub to_node_id: String,
    pub edge_type: PersistedEdgeType,
}

/// Full graph snapshot for periodic saves.
#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct GraphSnapshot {
    pub nodes: Vec<PersistedNode>,
    pub edges: Vec<PersistedEdge>,
    pub timestamp_secs: u64,
}

/// Log entry for mutation journaling.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LogEntry {
    AddNode {
        node_id: String,
        url: String,
        position_x: f32,
        position_y: f32,
    },
    AddEdge {
        from_node_id: String,
        to_node_id: String,
        edge_type: PersistedEdgeType,
    },
    RemoveEdge {
        from_node_id: String,
        to_node_id: String,
        edge_type: PersistedEdgeType,
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
    /// Update the address-kind classification of a node.
    UpdateNodeAddressKind {
        node_id: String,
        kind: PersistedAddressKind,
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
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            position_x: 100.0,
            position_y: 200.0,
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
            address_kind: PersistedAddressKind::Http,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&node).unwrap();
        let archived = rkyv::access::<ArchivedPersistedNode, rkyv::rancor::Error>(&bytes).unwrap();
        assert!(!archived.node_id.as_str().is_empty());
        assert_eq!(archived.url.as_str(), "https://example.com");
        assert_eq!(archived.title.as_str(), "Example");
        assert_eq!(archived.position_x, 100.0);
        assert_eq!(archived.position_y, 200.0);
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
        assert_eq!(archived.address_kind, ArchivedPersistedAddressKind::Http);
    }

    #[test]
    fn test_persisted_edge_roundtrip() {
        let edge = PersistedEdge {
            from_node_id: Uuid::new_v4().to_string(),
            to_node_id: Uuid::new_v4().to_string(),
            edge_type: PersistedEdgeType::Hyperlink,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&edge).unwrap();
        let archived = rkyv::access::<ArchivedPersistedEdge, rkyv::rancor::Error>(&bytes).unwrap();
        assert!(!archived.from_node_id.as_str().is_empty());
        assert!(!archived.to_node_id.as_str().is_empty());
        assert_eq!(archived.edge_type, ArchivedPersistedEdgeType::Hyperlink);
    }

    #[test]
    fn test_persisted_edge_roundtrip_user_grouped() {
        let edge = PersistedEdge {
            from_node_id: Uuid::new_v4().to_string(),
            to_node_id: Uuid::new_v4().to_string(),
            edge_type: PersistedEdgeType::UserGrouped,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&edge).unwrap();
        let archived = rkyv::access::<ArchivedPersistedEdge, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(archived.edge_type, ArchivedPersistedEdgeType::UserGrouped);
    }

    #[test]
    fn test_graph_snapshot_roundtrip() {
        let snapshot = GraphSnapshot {
            nodes: vec![PersistedNode {
                node_id: Uuid::new_v4().to_string(),
                url: "https://a.com".to_string(),
                title: "A".to_string(),
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
            edges: vec![],
            timestamp_secs: 1234567890,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot).unwrap();
        let archived = rkyv::access::<ArchivedGraphSnapshot, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(archived.nodes.len(), 1);
        assert_eq!(archived.edges.len(), 0);
        assert_eq!(archived.timestamp_secs, 1234567890);
    }

    #[test]
    fn test_log_entry_add_node_roundtrip() {
        let entry = LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "https://example.com".to_string(),
            position_x: 50.0,
            position_y: 75.0,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::AddNode {
                node_id,
                url,
                position_x,
                position_y,
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
            edge_type: PersistedEdgeType::UserGrouped,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        match archived {
            ArchivedLogEntry::RemoveEdge {
                from_node_id,
                to_node_id,
                edge_type,
            } => {
                assert!(!from_node_id.as_str().is_empty());
                assert!(!to_node_id.as_str().is_empty());
                assert_eq!(*edge_type, ArchivedPersistedEdgeType::UserGrouped);
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
    fn test_log_entry_update_node_address_kind_roundtrip() {
        for (kind, expected) in [
            (
                PersistedAddressKind::Http,
                ArchivedPersistedAddressKind::Http,
            ),
            (
                PersistedAddressKind::File,
                ArchivedPersistedAddressKind::File,
            ),
            (
                PersistedAddressKind::Custom,
                ArchivedPersistedAddressKind::Custom,
            ),
        ] {
            let entry = LogEntry::UpdateNodeAddressKind {
                node_id: Uuid::new_v4().to_string(),
                kind,
            };
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
            let archived = rkyv::access::<ArchivedLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
            match archived {
                ArchivedLogEntry::UpdateNodeAddressKind {
                    kind: archived_kind,
                    ..
                } => {
                    assert_eq!(*archived_kind, expected);
                }
                _ => panic!("Expected UpdateNodeAddressKind variant"),
            }
        }
    }

    #[test]
    fn test_persisted_address_kind_roundtrip() {
        for kind in [
            PersistedAddressKind::Http,
            PersistedAddressKind::File,
            PersistedAddressKind::Custom,
        ] {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&kind).unwrap();
            let archived =
                rkyv::access::<ArchivedPersistedAddressKind, rkyv::rancor::Error>(&bytes).unwrap();
            let expected = match kind {
                PersistedAddressKind::Http => ArchivedPersistedAddressKind::Http,
                PersistedAddressKind::File => ArchivedPersistedAddressKind::File,
                PersistedAddressKind::Custom => ArchivedPersistedAddressKind::Custom,
            };
            assert_eq!(*archived, expected);
        }
    }
}
