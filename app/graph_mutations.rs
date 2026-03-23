use super::*;
use crate::services::persistence::types::{
    PersistedArrangementSubKind, PersistedContainmentSubKind, PersistedEdgeAssertion,
    PersistedImportedSubKind, PersistedProvenanceSubKind, PersistedRelationSelector,
    PersistedSemanticSubKind,
};

fn edge_type_to_assertion(
    edge_type: crate::graph::EdgeType,
    edge_label: Option<String>,
) -> Option<crate::graph::EdgeAssertion> {
    match edge_type {
        crate::graph::EdgeType::Hyperlink => Some(crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::Hyperlink,
            label: edge_label,
            decay_progress: None,
        }),
        crate::graph::EdgeType::UserGrouped => Some(crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::UserGrouped,
            label: edge_label,
            decay_progress: None,
        }),
        crate::graph::EdgeType::AgentDerived { decay_progress } => {
            Some(crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::AgentDerived,
                label: edge_label,
                decay_progress: Some(decay_progress),
            })
        }
        crate::graph::EdgeType::ContainmentRelation(sub_kind) => {
            Some(crate::graph::EdgeAssertion::Containment { sub_kind })
        }
        crate::graph::EdgeType::ArrangementRelation(sub_kind) => {
            Some(crate::graph::EdgeAssertion::Arrangement { sub_kind })
        }
        crate::graph::EdgeType::History | crate::graph::EdgeType::ImportedRelation => None,
    }
}

fn edge_type_to_selector(
    edge_type: crate::graph::EdgeType,
) -> Option<crate::graph::RelationSelector> {
    match edge_type {
        crate::graph::EdgeType::Hyperlink => Some(crate::graph::RelationSelector::Semantic(
            crate::graph::SemanticSubKind::Hyperlink,
        )),
        crate::graph::EdgeType::UserGrouped => Some(crate::graph::RelationSelector::Semantic(
            crate::graph::SemanticSubKind::UserGrouped,
        )),
        crate::graph::EdgeType::AgentDerived { .. } => Some(
            crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::AgentDerived),
        ),
        crate::graph::EdgeType::ContainmentRelation(sub_kind) => {
            Some(crate::graph::RelationSelector::Containment(sub_kind))
        }
        crate::graph::EdgeType::ArrangementRelation(sub_kind) => {
            Some(crate::graph::RelationSelector::Arrangement(sub_kind))
        }
        crate::graph::EdgeType::ImportedRelation => None,
        crate::graph::EdgeType::History => Some(crate::graph::RelationSelector::Family(
            crate::graph::EdgeFamily::Traversal,
        )),
    }
}

fn persisted_assertion_from_graph_assertion(
    assertion: crate::graph::EdgeAssertion,
) -> PersistedEdgeAssertion {
    match assertion {
        crate::graph::EdgeAssertion::Semantic {
            sub_kind,
            label,
            decay_progress,
        } => PersistedEdgeAssertion::Semantic {
            sub_kind: match sub_kind {
                crate::graph::SemanticSubKind::Hyperlink => PersistedSemanticSubKind::Hyperlink,
                crate::graph::SemanticSubKind::UserGrouped => PersistedSemanticSubKind::UserGrouped,
                crate::graph::SemanticSubKind::AgentDerived => {
                    PersistedSemanticSubKind::AgentDerived
                }
                crate::graph::SemanticSubKind::Cites => PersistedSemanticSubKind::Cites,
                crate::graph::SemanticSubKind::Quotes => PersistedSemanticSubKind::Quotes,
                crate::graph::SemanticSubKind::Summarizes => PersistedSemanticSubKind::Summarizes,
                crate::graph::SemanticSubKind::Elaborates => PersistedSemanticSubKind::Elaborates,
                crate::graph::SemanticSubKind::ExampleOf => PersistedSemanticSubKind::ExampleOf,
                crate::graph::SemanticSubKind::Supports => PersistedSemanticSubKind::Supports,
                crate::graph::SemanticSubKind::Contradicts => PersistedSemanticSubKind::Contradicts,
                crate::graph::SemanticSubKind::Questions => PersistedSemanticSubKind::Questions,
                crate::graph::SemanticSubKind::SameEntityAs => {
                    PersistedSemanticSubKind::SameEntityAs
                }
                crate::graph::SemanticSubKind::DuplicateOf => PersistedSemanticSubKind::DuplicateOf,
                crate::graph::SemanticSubKind::CanonicalMirrorOf => {
                    PersistedSemanticSubKind::CanonicalMirrorOf
                }
                crate::graph::SemanticSubKind::DependsOn => PersistedSemanticSubKind::DependsOn,
                crate::graph::SemanticSubKind::Blocks => PersistedSemanticSubKind::Blocks,
                crate::graph::SemanticSubKind::NextStep => PersistedSemanticSubKind::NextStep,
            },
            label,
            agent_decay_progress: decay_progress,
        },
        crate::graph::EdgeAssertion::Containment { sub_kind } => {
            PersistedEdgeAssertion::Containment {
                sub_kind: match sub_kind {
                    crate::graph::ContainmentSubKind::UrlPath => {
                        PersistedContainmentSubKind::UrlPath
                    }
                    crate::graph::ContainmentSubKind::Domain => PersistedContainmentSubKind::Domain,
                    crate::graph::ContainmentSubKind::FileSystem => {
                        PersistedContainmentSubKind::FileSystem
                    }
                    crate::graph::ContainmentSubKind::UserFolder => {
                        PersistedContainmentSubKind::UserFolder
                    }
                    crate::graph::ContainmentSubKind::ClipSource => {
                        PersistedContainmentSubKind::ClipSource
                    }
                    crate::graph::ContainmentSubKind::NotebookSection => {
                        PersistedContainmentSubKind::NotebookSection
                    }
                    crate::graph::ContainmentSubKind::CollectionMember => {
                        PersistedContainmentSubKind::CollectionMember
                    }
                },
            }
        }
        crate::graph::EdgeAssertion::Arrangement { sub_kind } => {
            PersistedEdgeAssertion::Arrangement {
                sub_kind: match sub_kind {
                    crate::graph::ArrangementSubKind::FrameMember => {
                        PersistedArrangementSubKind::FrameMember
                    }
                    crate::graph::ArrangementSubKind::TileGroup => {
                        PersistedArrangementSubKind::TileGroup
                    }
                    crate::graph::ArrangementSubKind::SplitPair => {
                        PersistedArrangementSubKind::SplitPair
                    }
                },
            }
        }
        crate::graph::EdgeAssertion::Imported { sub_kind } => PersistedEdgeAssertion::Imported {
            sub_kind: match sub_kind {
                crate::graph::ImportedSubKind::BookmarkFolder => {
                    PersistedImportedSubKind::BookmarkFolder
                }
                crate::graph::ImportedSubKind::HistoryImport => {
                    PersistedImportedSubKind::HistoryImport
                }
                crate::graph::ImportedSubKind::RssMembership => {
                    PersistedImportedSubKind::RssMembership
                }
                crate::graph::ImportedSubKind::FileSystemImport => {
                    PersistedImportedSubKind::FileSystemImport
                }
                crate::graph::ImportedSubKind::ArchiveMembership => {
                    PersistedImportedSubKind::ArchiveMembership
                }
                crate::graph::ImportedSubKind::SharedCollection => {
                    PersistedImportedSubKind::SharedCollection
                }
            },
        },
        crate::graph::EdgeAssertion::Provenance { sub_kind } => {
            PersistedEdgeAssertion::Provenance {
                sub_kind: match sub_kind {
                    crate::graph::ProvenanceSubKind::ClippedFrom => {
                        PersistedProvenanceSubKind::ClippedFrom
                    }
                    crate::graph::ProvenanceSubKind::ExcerptedFrom => {
                        PersistedProvenanceSubKind::ExcerptedFrom
                    }
                    crate::graph::ProvenanceSubKind::SummarizedFrom => {
                        PersistedProvenanceSubKind::SummarizedFrom
                    }
                    crate::graph::ProvenanceSubKind::TranslatedFrom => {
                        PersistedProvenanceSubKind::TranslatedFrom
                    }
                    crate::graph::ProvenanceSubKind::RewrittenFrom => {
                        PersistedProvenanceSubKind::RewrittenFrom
                    }
                    crate::graph::ProvenanceSubKind::GeneratedFrom => {
                        PersistedProvenanceSubKind::GeneratedFrom
                    }
                    crate::graph::ProvenanceSubKind::ExtractedFrom => {
                        PersistedProvenanceSubKind::ExtractedFrom
                    }
                    crate::graph::ProvenanceSubKind::ImportedFromSource => {
                        PersistedProvenanceSubKind::ImportedFromSource
                    }
                },
            }
        }
    }
}

fn persisted_selector_from_graph_selector(
    selector: crate::graph::RelationSelector,
) -> Option<PersistedRelationSelector> {
    Some(match selector {
        crate::graph::RelationSelector::Family(family) => {
            PersistedRelationSelector::Family(match family {
                crate::graph::EdgeFamily::Semantic => {
                    crate::services::persistence::types::PersistedEdgeFamily::Semantic
                }
                crate::graph::EdgeFamily::Traversal => {
                    crate::services::persistence::types::PersistedEdgeFamily::Traversal
                }
                crate::graph::EdgeFamily::Containment => {
                    crate::services::persistence::types::PersistedEdgeFamily::Containment
                }
                crate::graph::EdgeFamily::Arrangement => {
                    crate::services::persistence::types::PersistedEdgeFamily::Arrangement
                }
                crate::graph::EdgeFamily::Imported => {
                    crate::services::persistence::types::PersistedEdgeFamily::Imported
                }
                crate::graph::EdgeFamily::Provenance => {
                    crate::services::persistence::types::PersistedEdgeFamily::Provenance
                }
            })
        }
        crate::graph::RelationSelector::Semantic(sub_kind) => {
            PersistedRelationSelector::Semantic(match sub_kind {
                crate::graph::SemanticSubKind::Hyperlink => PersistedSemanticSubKind::Hyperlink,
                crate::graph::SemanticSubKind::UserGrouped => PersistedSemanticSubKind::UserGrouped,
                crate::graph::SemanticSubKind::AgentDerived => {
                    PersistedSemanticSubKind::AgentDerived
                }
                crate::graph::SemanticSubKind::Cites => PersistedSemanticSubKind::Cites,
                crate::graph::SemanticSubKind::Quotes => PersistedSemanticSubKind::Quotes,
                crate::graph::SemanticSubKind::Summarizes => PersistedSemanticSubKind::Summarizes,
                crate::graph::SemanticSubKind::Elaborates => PersistedSemanticSubKind::Elaborates,
                crate::graph::SemanticSubKind::ExampleOf => PersistedSemanticSubKind::ExampleOf,
                crate::graph::SemanticSubKind::Supports => PersistedSemanticSubKind::Supports,
                crate::graph::SemanticSubKind::Contradicts => PersistedSemanticSubKind::Contradicts,
                crate::graph::SemanticSubKind::Questions => PersistedSemanticSubKind::Questions,
                crate::graph::SemanticSubKind::SameEntityAs => {
                    PersistedSemanticSubKind::SameEntityAs
                }
                crate::graph::SemanticSubKind::DuplicateOf => PersistedSemanticSubKind::DuplicateOf,
                crate::graph::SemanticSubKind::CanonicalMirrorOf => {
                    PersistedSemanticSubKind::CanonicalMirrorOf
                }
                crate::graph::SemanticSubKind::DependsOn => PersistedSemanticSubKind::DependsOn,
                crate::graph::SemanticSubKind::Blocks => PersistedSemanticSubKind::Blocks,
                crate::graph::SemanticSubKind::NextStep => PersistedSemanticSubKind::NextStep,
            })
        }
        crate::graph::RelationSelector::Containment(sub_kind) => {
            PersistedRelationSelector::Containment(match sub_kind {
                crate::graph::ContainmentSubKind::UrlPath => PersistedContainmentSubKind::UrlPath,
                crate::graph::ContainmentSubKind::Domain => PersistedContainmentSubKind::Domain,
                crate::graph::ContainmentSubKind::FileSystem => {
                    PersistedContainmentSubKind::FileSystem
                }
                crate::graph::ContainmentSubKind::UserFolder => {
                    PersistedContainmentSubKind::UserFolder
                }
                crate::graph::ContainmentSubKind::ClipSource => {
                    PersistedContainmentSubKind::ClipSource
                }
                crate::graph::ContainmentSubKind::NotebookSection => {
                    PersistedContainmentSubKind::NotebookSection
                }
                crate::graph::ContainmentSubKind::CollectionMember => {
                    PersistedContainmentSubKind::CollectionMember
                }
            })
        }
        crate::graph::RelationSelector::Arrangement(sub_kind) => {
            PersistedRelationSelector::Arrangement(match sub_kind {
                crate::graph::ArrangementSubKind::FrameMember => {
                    PersistedArrangementSubKind::FrameMember
                }
                crate::graph::ArrangementSubKind::TileGroup => {
                    PersistedArrangementSubKind::TileGroup
                }
                crate::graph::ArrangementSubKind::SplitPair => {
                    PersistedArrangementSubKind::SplitPair
                }
            })
        }
        crate::graph::RelationSelector::Imported(sub_kind) => {
            PersistedRelationSelector::Imported(match sub_kind {
                crate::graph::ImportedSubKind::BookmarkFolder => {
                    PersistedImportedSubKind::BookmarkFolder
                }
                crate::graph::ImportedSubKind::HistoryImport => {
                    PersistedImportedSubKind::HistoryImport
                }
                crate::graph::ImportedSubKind::RssMembership => {
                    PersistedImportedSubKind::RssMembership
                }
                crate::graph::ImportedSubKind::FileSystemImport => {
                    PersistedImportedSubKind::FileSystemImport
                }
                crate::graph::ImportedSubKind::ArchiveMembership => {
                    PersistedImportedSubKind::ArchiveMembership
                }
                crate::graph::ImportedSubKind::SharedCollection => {
                    PersistedImportedSubKind::SharedCollection
                }
            })
        }
        crate::graph::RelationSelector::Provenance(sub_kind) => {
            PersistedRelationSelector::Provenance(match sub_kind {
                crate::graph::ProvenanceSubKind::ClippedFrom => {
                    PersistedProvenanceSubKind::ClippedFrom
                }
                crate::graph::ProvenanceSubKind::ExcerptedFrom => {
                    PersistedProvenanceSubKind::ExcerptedFrom
                }
                crate::graph::ProvenanceSubKind::SummarizedFrom => {
                    PersistedProvenanceSubKind::SummarizedFrom
                }
                crate::graph::ProvenanceSubKind::TranslatedFrom => {
                    PersistedProvenanceSubKind::TranslatedFrom
                }
                crate::graph::ProvenanceSubKind::RewrittenFrom => {
                    PersistedProvenanceSubKind::RewrittenFrom
                }
                crate::graph::ProvenanceSubKind::GeneratedFrom => {
                    PersistedProvenanceSubKind::GeneratedFrom
                }
                crate::graph::ProvenanceSubKind::ExtractedFrom => {
                    PersistedProvenanceSubKind::ExtractedFrom
                }
                crate::graph::ProvenanceSubKind::ImportedFromSource => {
                    PersistedProvenanceSubKind::ImportedFromSource
                }
            })
        }
    })
}

/// Durable identifier for a rich note document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteId(uuid::Uuid);

impl NoteId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub(crate) fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: NoteId,
    pub title: String,
    pub linked_node: Option<NodeKey>,
    pub source_url: Option<String>,
    pub body: String,
    pub created_at: std::time::SystemTime,
    pub updated_at: std::time::SystemTime,
}

impl GraphBrowserApp {
    fn refresh_protocol_probe_for_node(&mut self, key: NodeKey, url: &str, enqueue_cancel: bool) {
        let protocol_resolution =
            crate::shell::desktop::runtime::registries::protocol::ProtocolRegistry::default()
                .resolve(url);
        let should_probe = matches!(
            crate::graph::address_kind_from_url(url),
            crate::graph::AddressKind::Http
        ) && protocol_resolution.inferred_mime_hint.is_none();
        if should_probe || enqueue_cancel {
            self.set_pending_protocol_probe(key, should_probe.then(|| url.to_string()));
        }
    }

    pub fn add_node_and_sync(
        &mut self,
        url: String,
        position: euclid::default::Point2D<f32>,
    ) -> NodeKey {
        let GraphDeltaResult::NodeAdded(key) =
            self.apply_graph_delta_and_sync(GraphDelta::AddNode {
                id: None,
                url: url.clone(),
                position,
            })
        else {
            unreachable!("add node delta must return NodeAdded");
        };
        if let Some(store) = &mut self.services.persistence
            && let Some(node) = self.workspace.domain.graph.get_node(key)
        {
            store.log_mutation(&LogEntry::AddNode {
                node_id: node.id.to_string(),
                url: url.clone(),
                position_x: position.x,
                position_y: position.y,
                timestamp_ms: Self::unix_timestamp_ms_now(),
            });
        }
        self.workspace.graph_runtime.physics.base.is_running = true;
        self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        self.refresh_protocol_probe_for_node(key, &url, false);
        key
    }

    pub(crate) fn preferred_new_node_anchor(&self, anchor: Option<NodeKey>) -> Option<NodeKey> {
        anchor.or_else(|| {
            self.focused_selection().primary().and_then(|key| {
                crate::shell::desktop::runtime::registries::phase3_suggest_semantic_placement_anchor(
                    self, key,
                )
            })
        })
    }

    pub(crate) fn suggested_new_node_position(
        &self,
        anchor: Option<NodeKey>,
    ) -> euclid::default::Point2D<f32> {
        let base = self
            .preferred_new_node_anchor(anchor)
            .and_then(|key| self.domain_graph().node_projected_position(key))
            .unwrap_or_else(|| {
                self.workspace
                    .domain
                    .graph
                    .projected_centroid()
                    .unwrap_or_else(|| euclid::default::Point2D::new(400.0, 300.0))
            });
        let n = self.domain_graph().node_count() as f32;
        let angle = n * std::f32::consts::FRAC_PI_4;
        let radius = 90.0;
        euclid::default::Point2D::new(base.x + radius * angle.cos(), base.y + radius * angle.sin())
    }

    pub fn add_edge_and_sync(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
        edge_label: Option<String>,
    ) -> Option<crate::graph::EdgeKey> {
        if let Some(assertion) = edge_type_to_assertion(edge_type, edge_label.clone()) {
            return self.assert_relation_and_sync(from_key, to_key, assertion);
        }
        let GraphDeltaResult::EdgeAdded(edge_key) =
            self.apply_graph_delta_and_sync(GraphDelta::AddEdge {
                from: from_key,
                to: to_key,
                edge_type,
                edge_label: edge_label.clone(),
            })
        else {
            unreachable!("add edge delta must return EdgeAdded");
        };
        if edge_key.is_some() {
            self.log_edge_mutation(from_key, to_key, edge_type, edge_label);
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        edge_key
    }

    pub fn assert_relation_and_sync(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        assertion: crate::graph::EdgeAssertion,
    ) -> Option<crate::graph::EdgeKey> {
        let GraphDeltaResult::EdgeAdded(edge_key) =
            self.apply_graph_delta_and_sync(GraphDelta::AssertRelation {
                from: from_key,
                to: to_key,
                assertion: assertion.clone(),
            })
        else {
            unreachable!("assert relation delta must return EdgeAdded");
        };
        if edge_key.is_some() {
            self.log_relation_assertion(from_key, to_key, assertion);
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        edge_key
    }

    pub fn remove_edges_and_log(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
    ) -> usize {
        if edge_type == crate::graph::EdgeType::History {
            let mut emitted_dissolved_append = false;
            let removed = if let Some(store) = &mut self.services.persistence {
                let dissolved_before = store.dissolved_archive_len();
                let removed = store
                    .dissolve_and_remove_edges(
                        &mut self.workspace.domain.graph,
                        from_key,
                        to_key,
                        edge_type,
                    )
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "Dissolution transfer failed, falling back to direct removal: {e}"
                        );
                        self.workspace
                            .domain
                            .graph
                            .remove_edges(from_key, to_key, edge_type)
                    });
                let dissolved_after = store.dissolved_archive_len();
                emitted_dissolved_append = dissolved_after > dissolved_before;
                removed
            } else {
                self.workspace
                    .domain
                    .graph
                    .remove_edges(from_key, to_key, edge_type)
            };

            if emitted_dissolved_append {
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                    latency_us: 0,
                });
            }

            if removed > 0 {
                self.log_edge_removal_mutation(from_key, to_key, edge_type);
                self.workspace.graph_runtime.egui_state_dirty = true;
                self.workspace.graph_runtime.physics.base.is_running = true;
                self.workspace.graph_runtime.drag_release_frames_remaining = 0;
            }
            return removed;
        }

        if let Some(selector) = edge_type_to_selector(edge_type) {
            return self.retract_relations_and_log(from_key, to_key, selector);
        }
        let mut emitted_dissolved_append = false;
        let removed = if let Some(store) = &mut self.services.persistence {
            let dissolved_before = store.dissolved_archive_len();
            let removed = store
                .dissolve_and_remove_edges(
                    &mut self.workspace.domain.graph,
                    from_key,
                    to_key,
                    edge_type,
                )
                .unwrap_or_else(|e| {
                    log::warn!("Dissolution transfer failed, falling back to direct removal: {e}");
                    self.workspace
                        .domain
                        .graph
                        .remove_edges(from_key, to_key, edge_type)
                });
            let dissolved_after = store.dissolved_archive_len();
            emitted_dissolved_append = dissolved_after > dissolved_before;
            removed
        } else {
            self.workspace
                .domain
                .graph
                .remove_edges(from_key, to_key, edge_type)
        };

        if emitted_dissolved_append {
            self.workspace.graph_runtime.history_last_event_unix_ms =
                Some(Self::unix_timestamp_ms_now());
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                latency_us: 0,
            });
        }

        if removed > 0 {
            self.log_edge_removal_mutation(from_key, to_key, edge_type);
            self.workspace.graph_runtime.egui_state_dirty = true;
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        removed
    }

    pub fn retract_relations_and_log(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        selector: crate::graph::RelationSelector,
    ) -> usize {
        let GraphDeltaResult::EdgesRemoved(removed) =
            self.apply_graph_delta_and_sync(GraphDelta::RetractRelations {
                from: from_key,
                to: to_key,
                selector,
            })
        else {
            unreachable!("retract relations delta must return EdgesRemoved");
        };
        if removed > 0 {
            self.log_relation_retraction(from_key, to_key, selector);
            self.workspace.graph_runtime.egui_state_dirty = true;
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        removed
    }

    fn log_relation_assertion(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        assertion: crate::graph::EdgeAssertion,
    ) {
        if let Some(store) = &mut self.services.persistence {
            let from_id = self
                .workspace
                .domain
                .graph
                .get_node(from_key)
                .map(|n| n.id.to_string());
            let to_id = self
                .workspace
                .domain
                .graph
                .get_node(to_key)
                .map(|n| n.id.to_string());
            let (Some(from_node_id), Some(to_node_id)) = (from_id, to_id) else {
                return;
            };
            store.log_mutation(&LogEntry::AddEdge {
                from_node_id,
                to_node_id,
                assertion: persisted_assertion_from_graph_assertion(assertion),
            });
        }
    }

    fn log_relation_retraction(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        selector: crate::graph::RelationSelector,
    ) {
        if let Some(store) = &mut self.services.persistence {
            let from_id = self
                .workspace
                .domain
                .graph
                .get_node(from_key)
                .map(|n| n.id.to_string());
            let to_id = self
                .workspace
                .domain
                .graph
                .get_node(to_key)
                .map(|n| n.id.to_string());
            let (Some(from_node_id), Some(to_node_id)) = (from_id, to_id) else {
                return;
            };
            let Some(selector) = persisted_selector_from_graph_selector(selector) else {
                return;
            };
            store.log_mutation(&LogEntry::RemoveEdge {
                from_node_id,
                to_node_id,
                selector,
            });
        }
    }

    pub fn log_edge_mutation(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
        edge_label: Option<String>,
    ) {
        if let Some(assertion) = edge_type_to_assertion(edge_type, edge_label) {
            self.log_relation_assertion(from_key, to_key, assertion);
        }
    }

    pub fn log_edge_removal_mutation(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
    ) {
        if let Some(selector) = edge_type_to_selector(edge_type) {
            self.log_relation_retraction(from_key, to_key, selector);
        }
    }

    pub fn log_title_mutation(&mut self, node_key: NodeKey) {
        if let Some(store) = &mut self.services.persistence {
            if let Some(node) = self.workspace.domain.graph.get_node(node_key) {
                let node_id = node.id.to_string();
                let title = node.title.clone();
                store.log_mutation(&LogEntry::UpdateNodeTitle {
                    node_id: node_id.clone(),
                    title: title.clone(),
                });
                store.log_audit_event(
                    &node_id,
                    crate::services::persistence::types::NodeAuditEventKind::TitleChanged {
                        new_title: title,
                    },
                    Self::unix_timestamp_ms_now(),
                );
            }
        }
    }

    pub(crate) fn maybe_add_history_traversal_edge(
        &mut self,
        node_key: NodeKey,
        old_entries: &[String],
        old_index: usize,
        new_entries: &[String],
        new_index: usize,
    ) {
        let Some(old_url) = old_entries.get(old_index).filter(|url| !url.is_empty()) else {
            self.record_history_failure(
                HistoryTraversalFailureReason::MissingOldUrl,
                "old history entry missing or empty",
            );
            return;
        };
        let Some(new_url) = new_entries.get(new_index).filter(|url| !url.is_empty()) else {
            self.record_history_failure(
                HistoryTraversalFailureReason::MissingNewUrl,
                "new history entry missing or empty",
            );
            return;
        };
        if old_url == new_url {
            self.record_history_failure(
                HistoryTraversalFailureReason::SameUrl,
                "history transition resolves to same URL",
            );
            return;
        }

        let is_back = new_index < old_index;
        let is_forward_same_list = new_index > old_index && new_entries.len() == old_entries.len();
        if !is_back && !is_forward_same_list {
            self.record_history_failure(
                HistoryTraversalFailureReason::NonHistoryTransition,
                "transition is not a back/forward history move",
            );
            return;
        }
        let trigger = if is_back {
            NavigationTrigger::Back
        } else {
            NavigationTrigger::Forward
        };

        let from_key = self
            .workspace
            .domain
            .graph
            .get_nodes_by_url(old_url)
            .into_iter()
            .find(|&key| key != node_key)
            .or(Some(node_key));
        let to_key = self
            .workspace
            .domain
            .graph
            .get_nodes_by_url(new_url)
            .into_iter()
            .find(|&key| key != node_key)
            .or(Some(node_key));
        let (Some(from_key), Some(to_key)) = (from_key, to_key) else {
            self.record_history_failure(
                HistoryTraversalFailureReason::MissingEndpoint,
                "could not resolve traversal endpoints",
            );
            return;
        };

        let _ = self.push_history_traversal_and_sync(from_key, to_key, trigger);
    }

    pub(crate) fn push_history_traversal_and_sync(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        trigger: NavigationTrigger,
    ) -> bool {
        if from_key == to_key {
            self.record_history_failure(
                HistoryTraversalFailureReason::SelfLoop,
                "from_key equals to_key",
            );
            return false;
        }
        let existing_edge_key = self.workspace.domain.graph.find_edge_key(from_key, to_key);
        let history_semantic_existed = existing_edge_key
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .map(|payload| payload.has_edge_type(EdgeType::History))
            .unwrap_or(false);

        let traversal = Traversal::now(trigger);
        let GraphDeltaResult::TraversalAppended(appended) =
            self.apply_graph_delta_and_sync(GraphDelta::AppendTraversal {
                from: from_key,
                to: to_key,
                traversal,
            })
        else {
            unreachable!("append traversal delta must return TraversalAppended");
        };
        if !appended {
            self.record_history_failure(
                HistoryTraversalFailureReason::GraphRejected,
                "graph push_traversal rejected append",
            );
            return false;
        }

        self.workspace.graph_runtime.history_last_event_unix_ms =
            Some(Self::unix_timestamp_ms_now());

        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_HISTORY_TRAVERSAL_RECORDED,
            latency_us: 0,
        });

        if !history_semantic_existed {
            self.log_edge_mutation(from_key, to_key, EdgeType::History, None);
        }
        self.log_traversal_mutation(from_key, to_key, traversal);
        self.workspace.graph_runtime.physics.base.is_running = true;
        self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        true
    }

    fn log_traversal_mutation(&mut self, from_key: NodeKey, to_key: NodeKey, traversal: Traversal) {
        if let Some(store) = &mut self.services.persistence {
            let from_id = self
                .workspace
                .domain
                .graph
                .get_node(from_key)
                .map(|n| n.id.to_string());
            let to_id = self
                .workspace
                .domain
                .graph
                .get_node(to_key)
                .map(|n| n.id.to_string());
            let (Some(from_node_id), Some(to_node_id)) = (from_id, to_id) else {
                return;
            };
            let trigger = match traversal.trigger {
                NavigationTrigger::Unknown => PersistedNavigationTrigger::Unknown,
                NavigationTrigger::LinkClick => PersistedNavigationTrigger::LinkClick,
                NavigationTrigger::Back => PersistedNavigationTrigger::Back,
                NavigationTrigger::Forward => PersistedNavigationTrigger::Forward,
                NavigationTrigger::AddressBarEntry => PersistedNavigationTrigger::AddressBarEntry,
                NavigationTrigger::PanePromotion => PersistedNavigationTrigger::PanePromotion,
                NavigationTrigger::Programmatic => PersistedNavigationTrigger::Programmatic,
            };
            store.log_mutation(&LogEntry::AppendTraversal {
                from_node_id,
                to_node_id,
                timestamp_ms: traversal.timestamp_ms,
                trigger,
            });
        }
    }

    pub(crate) fn unix_timestamp_ms_now() -> u64 {
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub(crate) fn record_history_failure(
        &mut self,
        reason: HistoryTraversalFailureReason,
        detail: impl Into<String>,
    ) {
        self.update_history_failure(reason, detail)
    }

    pub(crate) fn add_user_grouped_edge_if_missing(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        label: Option<String>,
    ) {
        if from == to {
            return;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return;
        }
        let already_grouped = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .is_some_and(|payload| {
                payload.has_relation(crate::graph::RelationSelector::Semantic(
                    crate::graph::SemanticSubKind::UserGrouped,
                ))
            });
        if !already_grouped {
            let _ = self.assert_relation_and_sync(
                from,
                to,
                crate::graph::EdgeAssertion::Semantic {
                    sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                    label,
                    decay_progress: None,
                },
            );
        }
    }

    pub(crate) fn delete_import_record(&mut self, record_id: String) {
        if self.workspace.domain.graph.delete_import_record(&record_id) {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
    }

    pub(crate) fn suppress_import_record_membership(&mut self, record_id: String, key: NodeKey) {
        if self
            .workspace
            .domain
            .graph
            .set_import_record_membership_suppressed(&record_id, key, true)
        {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
    }

    pub(crate) fn promote_import_record_to_user_group(
        &mut self,
        record_id: String,
        anchor: NodeKey,
    ) {
        let member_keys = self
            .workspace
            .domain
            .graph
            .import_record_member_keys(&record_id);
        if !member_keys.contains(&anchor) {
            return;
        }
        for member in member_keys {
            if member == anchor {
                continue;
            }
            self.add_user_grouped_edge_if_missing(anchor, member, None);
            self.add_user_grouped_edge_if_missing(member, anchor, None);
        }
    }

    pub(crate) fn add_arrangement_relation_if_missing(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        sub_kind: crate::graph::ArrangementSubKind,
    ) {
        if from == to {
            return;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return;
        }
        let selector = crate::graph::RelationSelector::Arrangement(sub_kind);
        let has_opposite_durability = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .and_then(|payload| payload.arrangement_data())
            .is_some_and(|arrangement| {
                arrangement
                    .sub_kinds
                    .iter()
                    .copied()
                    .any(|existing| existing.durability() != sub_kind.durability())
            });
        let already_exists = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .is_some_and(|payload| payload.has_relation(selector));
        if !already_exists {
            let _ = self.assert_relation_and_sync(
                from,
                to,
                crate::graph::EdgeAssertion::Arrangement { sub_kind },
            );
            if has_opposite_durability {
                self.emit_arrangement_durability_transition();
            }
        }
    }

    pub(crate) fn promote_arrangement_relation_to_frame_membership(
        &mut self,
        from: NodeKey,
        to: NodeKey,
    ) {
        if from == to {
            return;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return;
        }
        let edge_payload = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key));
        let had_tile_group = edge_payload.is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Arrangement(
                crate::graph::ArrangementSubKind::TileGroup,
            ))
        });
        let had_split_pair = edge_payload.is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Arrangement(
                crate::graph::ArrangementSubKind::SplitPair,
            ))
        });
        let had_frame_member = edge_payload.is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Arrangement(
                crate::graph::ArrangementSubKind::FrameMember,
            ))
        });

        self.add_arrangement_relation_if_missing(
            from,
            to,
            crate::graph::ArrangementSubKind::FrameMember,
        );

        if had_tile_group {
            let _ = self.retract_relations_and_log(
                from,
                to,
                crate::graph::RelationSelector::Arrangement(
                    crate::graph::ArrangementSubKind::TileGroup,
                ),
            );
        }
        if had_split_pair {
            let _ = self.retract_relations_and_log(
                from,
                to,
                crate::graph::RelationSelector::Arrangement(
                    crate::graph::ArrangementSubKind::SplitPair,
                ),
            );
        }

        if (had_tile_group || had_split_pair) && had_frame_member {
            self.emit_arrangement_durability_transition();
        }
    }

    pub(crate) fn create_user_grouped_edge_from_primary_selection(&mut self) {
        let selection = self.focused_selection();
        let Some(from) = selection.primary() else {
            return;
        };
        let to = selection.iter().copied().find(|key| *key != from);
        if let Some(to) = to {
            self.add_user_grouped_edge_if_missing(from, to, None);
        }
    }

    pub(crate) fn group_nodes_by_semantic_tags(&mut self) {
        use std::collections::{HashMap, HashSet};

        let mut clusters: HashMap<u8, HashSet<NodeKey>> = HashMap::new();

        for (&node_key, vector) in &self.workspace.graph_runtime.semantic_index {
            for code in &vector.classes {
                if let Some(&first_digit) = code.0.first() {
                    clusters.entry(first_digit).or_default().insert(node_key);
                }
            }
        }

        let mut created_pairs = std::collections::HashSet::new();

        for (_subject_code, nodes) in clusters {
            let nodes: Vec<NodeKey> = nodes.into_iter().collect();
            if nodes.len() < 2 {
                continue;
            }

            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    let (a, b) = (nodes[i], nodes[j]);
                    let pair = if a < b { (a, b) } else { (b, a) };
                    if !created_pairs.contains(&pair) {
                        created_pairs.insert(pair);
                        self.add_user_grouped_edge_if_missing(a, b, None);
                        self.add_user_grouped_edge_if_missing(b, a, None);
                    }
                }
            }
        }
    }

    pub(crate) fn selected_pair_in_order(&self) -> Option<(NodeKey, NodeKey)> {
        self.focused_selection().ordered_pair()
    }

    pub(crate) fn intents_for_edge_command(&self, command: EdgeCommand) -> Vec<GraphIntent> {
        match command {
            EdgeCommand::ConnectSelectedPair => self
                .selected_pair_in_order()
                .map(|(from, to)| {
                    vec![GraphIntent::CreateUserGroupedEdge {
                        from,
                        to,
                        label: None,
                    }]
                })
                .unwrap_or_default(),
            EdgeCommand::ConnectPair { from, to } => {
                vec![GraphIntent::CreateUserGroupedEdge {
                    from,
                    to,
                    label: None,
                }]
            }
            EdgeCommand::ConnectBothDirections => self
                .selected_pair_in_order()
                .map(|(from, to)| {
                    vec![
                        GraphIntent::CreateUserGroupedEdge {
                            from,
                            to,
                            label: None,
                        },
                        GraphIntent::CreateUserGroupedEdge {
                            from: to,
                            to: from,
                            label: None,
                        },
                    ]
                })
                .unwrap_or_default(),
            EdgeCommand::ConnectBothDirectionsPair { a, b } => {
                vec![
                    GraphIntent::CreateUserGroupedEdge {
                        from: a,
                        to: b,
                        label: None,
                    },
                    GraphIntent::CreateUserGroupedEdge {
                        from: b,
                        to: a,
                        label: None,
                    },
                ]
            }
            EdgeCommand::RemoveUserEdge => self
                .selected_pair_in_order()
                .map(|(from, to)| {
                    vec![
                        GraphIntent::RemoveEdge {
                            from,
                            to,
                            selector: crate::graph::RelationSelector::Semantic(
                                crate::graph::SemanticSubKind::UserGrouped,
                            ),
                        },
                        GraphIntent::RemoveEdge {
                            from: to,
                            to: from,
                            selector: crate::graph::RelationSelector::Semantic(
                                crate::graph::SemanticSubKind::UserGrouped,
                            ),
                        },
                    ]
                })
                .unwrap_or_default(),
            EdgeCommand::RemoveUserEdgePair { a, b } => {
                vec![
                    GraphIntent::RemoveEdge {
                        from: a,
                        to: b,
                        selector: crate::graph::RelationSelector::Semantic(
                            crate::graph::SemanticSubKind::UserGrouped,
                        ),
                    },
                    GraphIntent::RemoveEdge {
                        from: b,
                        to: a,
                        selector: crate::graph::RelationSelector::Semantic(
                            crate::graph::SemanticSubKind::UserGrouped,
                        ),
                    },
                ]
            }
            EdgeCommand::PinSelected => self
                .focused_selection()
                .iter()
                .copied()
                .map(|key| GraphIntent::SetNodePinned {
                    key,
                    is_pinned: true,
                })
                .collect(),
            EdgeCommand::UnpinSelected => self
                .focused_selection()
                .iter()
                .copied()
                .map(|key| GraphIntent::SetNodePinned {
                    key,
                    is_pinned: false,
                })
                .collect(),
        }
    }

    pub(crate) fn set_node_pinned_and_log(&mut self, key: NodeKey, is_pinned: bool) {
        let Some(current_state) = self
            .workspace
            .domain
            .graph
            .get_node(key)
            .map(|node| node.is_pinned)
        else {
            return;
        };
        let had_pin_tag = self
            .workspace
            .domain
            .graph
            .node_tags(key)
            .is_some_and(|tags| tags.contains(Self::TAG_PIN));
        if current_state == is_pinned && had_pin_tag == is_pinned {
            return;
        }

        let _ = self.apply_graph_delta_and_sync(GraphDelta::SetNodePinned { key, is_pinned });

        let tags_changed = if is_pinned {
            self.workspace
                .domain
                .graph
                .insert_node_tag(key, Self::TAG_PIN.to_string())
        } else {
            self.workspace
                .domain
                .graph
                .remove_node_tag(key, Self::TAG_PIN)
        };

        if tags_changed {
            self.workspace.graph_runtime.semantic_index_dirty = true;
        }

        if let Some(store) = &mut self.services.persistence {
            let node_id = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.id.to_string())
                .unwrap_or_default();
            store.log_mutation(&LogEntry::PinNode {
                node_id: node_id.clone(),
                is_pinned,
            });
            let audit_event = if is_pinned {
                crate::services::persistence::types::NodeAuditEventKind::Pinned
            } else {
                crate::services::persistence::types::NodeAuditEventKind::Unpinned
            };
            store.log_audit_event(&node_id, audit_event, Self::unix_timestamp_ms_now());
        }
    }

    pub fn create_new_node_near_center(&mut self) -> NodeKey {
        let position = self.suggested_new_node_position(None);
        let placeholder_url = self.next_placeholder_url();

        let key = self.add_node_and_sync(placeholder_url, position);
        self.select_node(key, false);
        key
    }

    pub fn remove_selected_nodes(&mut self) {
        let nodes_to_remove: Vec<NodeKey> = self.focused_selection().iter().copied().collect();

        for node_key in nodes_to_remove {
            let node_id = self
                .workspace
                .domain
                .graph
                .get_node(node_key)
                .map(|node| node.id);

            if let Some(store) = &mut self.services.persistence {
                if let Some(node_id) = node_id {
                    store.log_mutation(&LogEntry::RemoveNode {
                        node_id: node_id.to_string(),
                        timestamp_ms: Self::unix_timestamp_ms_now(),
                    });
                }
            }

            if let Some(webview_id) = self
                .workspace
                .graph_runtime
                .node_to_webview
                .get(&node_key)
                .copied()
            {
                let _ = self.unmap_webview(webview_id);
            }
            self.remove_active_node(node_key);
            self.remove_warm_cache_node(node_key);
            self.workspace
                .graph_runtime
                .runtime_block_state
                .remove(&node_key);
            self.workspace
                .graph_runtime
                .runtime_block_state
                .remove(&node_key);
            self.workspace
                .graph_runtime
                .suggested_semantic_tags
                .remove(&node_key);
            if let Some(node_id) = node_id {
                self.workspace.workbench_session.on_node_deleted(node_id);
            }

            if let Some(store) = &mut self.services.persistence {
                let dissolved_before = store.dissolved_archive_len();
                let _ = store.dissolve_and_remove_node(&mut self.workspace.domain.graph, node_key);
                let dissolved_after = store.dissolved_archive_len();
                if dissolved_after > dissolved_before {
                    self.workspace.graph_runtime.history_last_event_unix_ms =
                        Some(Self::unix_timestamp_ms_now());
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                        latency_us: 0,
                    });
                }
            } else {
                let _ = self.apply_graph_delta_and_sync(GraphDelta::RemoveNode { key: node_key });
            }
        }

        self.clear_selection();
        self.prune_selection_to_existing_nodes();
        self.workspace.graph_runtime.highlighted_graph_edge = None;
        let pending_node_context_target = self
            .pending_node_context_target()
            .filter(|key| self.workspace.domain.graph.get_node(*key).is_some());
        self.set_pending_node_context_target(pending_node_context_target);
        self.sanitize_pending_frame_import_commands();
    }

    /// Soft-delete selected nodes: transitions them to `NodeLifecycle::Tombstone`
    /// (Ghost Node) without removing them from the graph.  Webview resources are
    /// freed; the node remains structurally present for topology preservation.
    pub fn mark_tombstone_for_selected(&mut self) {
        use crate::graph::NodeLifecycle;
        let nodes: Vec<NodeKey> = self.focused_selection().iter().copied().collect();
        for key in nodes {
            let already_tombstone = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|n| n.lifecycle == NodeLifecycle::Tombstone);
            if already_tombstone {
                continue;
            }
            // Free webview resources like a cold demotion.
            if let Some(webview_id) = self
                .workspace
                .graph_runtime
                .node_to_webview
                .get(&key)
                .copied()
            {
                let _ = self.unmap_webview(webview_id);
            }
            self.remove_active_node(key);
            self.remove_warm_cache_node(key);
            self.workspace
                .domain
                .graph
                .set_node_lifecycle(key, NodeLifecycle::Tombstone);
        }
        self.clear_selection();
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    /// Restore a single Ghost Node from `NodeLifecycle::Tombstone → Cold`.
    /// The node retains its preserved position and edges.
    pub fn restore_ghost_node(&mut self, key: NodeKey) {
        use crate::graph::NodeLifecycle;
        let is_tombstone = self
            .workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|n| n.lifecycle == NodeLifecycle::Tombstone);
        if !is_tombstone {
            return;
        }
        self.workspace
            .domain
            .graph
            .set_node_lifecycle(key, NodeLifecycle::Cold);
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn get_single_selected_node(&self) -> Option<NodeKey> {
        let selected = self.focused_selection();
        if selected.len() == 1 {
            selected.primary()
        } else {
            None
        }
    }

    pub(crate) fn suggested_semantic_tags_for_node(&self, key: NodeKey) -> Vec<String> {
        self.workspace
            .graph_runtime
            .suggested_semantic_tags
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    pub fn clear_graph(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            store.log_mutation(&LogEntry::ClearGraph);
        }
        self.workspace.domain.graph = Graph::new();
        self.reset_selection_state();
        self.workspace.graph_runtime.highlighted_graph_edge = None;
        self.workspace.graph_runtime.navigator_projection_state =
            NavigatorProjectionState::default();
        self.clear_choose_frame_picker();
        self.workspace
            .workbench_session
            .pending_app_commands
            .clear();
        self.clear_pending_camera_command();
        self.clear_pending_wheel_zoom_delta();
        self.workspace.domain.notes.clear();
        self.workspace.graph_runtime.views.clear();
        self.workspace.graph_runtime.graph_view_frames.clear();
        self.workspace.graph_runtime.graph_view_canvas_rects.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.graph_runtime.webview_to_node.clear();
        self.workspace.graph_runtime.node_to_webview.clear();
        self.workspace.graph_runtime.active_lru.clear();
        self.workspace.graph_runtime.warm_cache_lru.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.suggested_semantic_tags.clear();
        self.workspace.graph_runtime.semantic_index.clear();
        self.workspace.graph_runtime.semantic_index_dirty = true;
        self.workspace
            .workbench_session
            .node_last_active_workspace
            .clear();
        self.workspace
            .workbench_session
            .node_workspace_membership
            .clear();
        self.workspace
            .workbench_session
            .last_session_workspace_layout_hash = None;
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json = None;
        self.workspace.workbench_session.last_workspace_autosave_at = None;
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn clear_graph_and_persistence(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            if let Err(e) = store.clear_all() {
                warn!("Failed to clear persisted graph data: {e}");
            }
        }
        self.workspace.domain.graph = Graph::new();
        self.reset_selection_state();
        self.workspace.graph_runtime.highlighted_graph_edge = None;
        self.workspace.graph_runtime.navigator_projection_state =
            NavigatorProjectionState::default();
        self.clear_choose_frame_picker();
        self.workspace
            .workbench_session
            .pending_app_commands
            .clear();
        self.clear_pending_camera_command();
        self.clear_pending_wheel_zoom_delta();
        self.workspace.graph_runtime.views.clear();
        self.workspace.graph_runtime.graph_view_frames.clear();
        self.workspace.graph_runtime.graph_view_canvas_rects.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.graph_runtime.webview_to_node.clear();
        self.workspace.graph_runtime.node_to_webview.clear();
        self.workspace.graph_runtime.active_lru.clear();
        self.workspace.graph_runtime.warm_cache_lru.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.suggested_semantic_tags.clear();
        self.workspace
            .workbench_session
            .node_last_active_workspace
            .clear();
        self.workspace
            .workbench_session
            .node_workspace_membership
            .clear();
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.graph_runtime.active_webview_nodes.clear();
        self.workspace.domain.next_placeholder_id = 0;
        self.workspace.graph_runtime.egui_state_dirty = true;
        self.workspace.graph_runtime.semantic_index.clear();
        self.workspace.graph_runtime.semantic_index_dirty = true;
    }

    pub fn update_node_url_and_log(&mut self, key: NodeKey, new_url: String) -> Option<String> {
        let new_mime_hint = crate::graph::detect_mime(&new_url, None);
        let new_address_kind = crate::graph::address_kind_from_url(&new_url);

        let GraphDeltaResult::NodeUrlUpdated(old_url) =
            self.apply_graph_delta_and_sync(GraphDelta::SetNodeUrl {
                key,
                new_url: new_url.clone(),
            })
        else {
            unreachable!("url delta must return NodeUrlUpdated");
        };
        let old_url = old_url?;

        let _ = self.apply_graph_delta_and_sync(GraphDelta::SetNodeMimeHint {
            key,
            mime_hint: new_mime_hint.clone(),
        });
        let _ = self.apply_graph_delta_and_sync(GraphDelta::SetNodeAddressKind {
            key,
            kind: new_address_kind,
        });

        if let Some(store) = &mut self.services.persistence {
            if let Some(node) = self.workspace.domain.graph.get_node(key) {
                let node_id = node.id.to_string();
                let ts = Self::unix_timestamp_ms_now();
                store.log_mutation(&LogEntry::NavigateNode {
                    node_id: node_id.clone(),
                    from_url: old_url.clone(),
                    to_url: new_url.clone(),
                    trigger: PersistedNavigationTrigger::Unknown,
                    timestamp_ms: ts,
                });
                store.log_mutation(&LogEntry::UpdateNodeUrl {
                    node_id: node_id.clone(),
                    new_url: new_url.clone(),
                });
                store.log_audit_event(
                    &node_id,
                    crate::services::persistence::types::NodeAuditEventKind::UrlChanged {
                        new_url: new_url.clone(),
                    },
                    ts,
                );
                store.log_mutation(&LogEntry::UpdateNodeMimeHint {
                    node_id: node_id.clone(),
                    mime_hint: new_mime_hint,
                });
                let persisted_kind = match new_address_kind {
                    crate::graph::AddressKind::Http => {
                        crate::services::persistence::types::PersistedAddressKind::Http
                    }
                    crate::graph::AddressKind::File => {
                        crate::services::persistence::types::PersistedAddressKind::File
                    }
                    crate::graph::AddressKind::Custom => {
                        crate::services::persistence::types::PersistedAddressKind::Custom
                    }
                };
                store.log_mutation(&LogEntry::UpdateNodeAddressKind {
                    node_id,
                    kind: persisted_kind,
                });
            }
        }
        self.workspace.graph_runtime.egui_state_dirty = true;
        self.refresh_protocol_probe_for_node(key, &new_url, true);
        Some(old_url)
    }

    pub fn create_note_for_node(&mut self, key: NodeKey, title: Option<String>) -> Option<NoteId> {
        let node = self.workspace.domain.graph.get_node(key)?;
        let now = SystemTime::now();
        let note_id = NoteId::new();
        let resolved_title = title.unwrap_or_else(|| {
            let base = node.title.trim();
            if base.is_empty() {
                format!("Note for {}", node.url)
            } else {
                format!("Note for {base}")
            }
        });
        let note = NoteRecord {
            id: note_id,
            title: resolved_title,
            linked_node: Some(key),
            source_url: Some(node.url.clone()),
            body: String::new(),
            created_at: now,
            updated_at: now,
        };

        self.workspace.domain.notes.insert(note_id, note);
        self.enqueue_app_command(AppCommand::OpenNote { note_id });
        self.request_open_node_tile_mode(key, PendingTileOpenMode::SplitHorizontal);
        Some(note_id)
    }

    pub fn note_record(&self, note_id: NoteId) -> Option<&NoteRecord> {
        self.workspace.domain.notes.get(&note_id)
    }

    pub(crate) fn apply_graph_delta_and_sync(&mut self, delta: GraphDelta) -> GraphDeltaResult {
        let result = apply_domain_graph_delta(&mut self.workspace.domain.graph, delta.clone());
        if Self::graph_structure_changed(&result) {
            self.clear_hop_distance_cache();
        }
        // Rebuild derived containment edges whenever the node set or a node's URL changes,
        // so ContainmentRelation edges stay consistent without requiring an explicit refresh.
        if Self::containment_affected(&result) {
            self.workspace
                .domain
                .graph
                .rebuild_derived_containment_relations();
        }
        if let Some(egui_state) = self.workspace.graph_runtime.egui_state.as_mut()
            && !egui_state.sync_from_delta(&self.workspace.domain.graph, &delta, &result)
        {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
        result
    }

    pub(crate) fn containment_affected(result: &GraphDeltaResult) -> bool {
        matches!(
            result,
            GraphDeltaResult::NodeAdded(_)
                | GraphDeltaResult::NodeMaybeAdded(Some(_))
                | GraphDeltaResult::NodeRemoved(true)
                | GraphDeltaResult::NodeUrlUpdated(Some(_))
        )
    }

    pub(crate) fn graph_structure_changed(result: &GraphDeltaResult) -> bool {
        match result {
            GraphDeltaResult::NodeAdded(_) => true,
            GraphDeltaResult::NodeMaybeAdded(maybe) => maybe.is_some(),
            GraphDeltaResult::EdgeAdded(maybe) => maybe.is_some(),
            GraphDeltaResult::NodeRemoved(changed) => *changed,
            GraphDeltaResult::EdgesRemoved(count) => *count > 0,
            GraphDeltaResult::TraversalAppended(_) => false,
            GraphDeltaResult::NodeMetadataUpdated(_) => false,
            GraphDeltaResult::NodeUrlUpdated(_) => false,
        }
    }
}
