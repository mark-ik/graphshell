use super::*;

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
            });
        }
        self.workspace.physics.base.is_running = true;
        self.workspace.drag_release_frames_remaining = 0;
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
            self.workspace.physics.base.is_running = true;
            self.workspace.drag_release_frames_remaining = 0;
        }
        edge_key
    }

    pub fn remove_edges_and_log(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
    ) -> usize {
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
            self.workspace.history_last_event_unix_ms = Some(Self::unix_timestamp_ms_now());
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                latency_us: 0,
            });
        }

        if removed > 0 {
            self.log_edge_removal_mutation(from_key, to_key, edge_type);
            self.workspace.egui_state_dirty = true;
            self.workspace.physics.base.is_running = true;
            self.workspace.drag_release_frames_remaining = 0;
        }
        removed
    }

    pub fn log_edge_mutation(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
        edge_label: Option<String>,
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
            let persisted_type = match edge_type {
                crate::graph::EdgeType::Hyperlink => PersistedEdgeType::Hyperlink,
                crate::graph::EdgeType::History => PersistedEdgeType::History,
                crate::graph::EdgeType::UserGrouped => PersistedEdgeType::UserGrouped,
            };
            store.log_mutation(&LogEntry::AddEdge {
                from_node_id,
                to_node_id,
                edge_type: persisted_type,
                edge_label,
            });
        }
    }

    pub fn log_edge_removal_mutation(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
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
            let persisted_type = match edge_type {
                crate::graph::EdgeType::Hyperlink => PersistedEdgeType::Hyperlink,
                crate::graph::EdgeType::History => PersistedEdgeType::History,
                crate::graph::EdgeType::UserGrouped => PersistedEdgeType::UserGrouped,
            };
            store.log_mutation(&LogEntry::RemoveEdge {
                from_node_id,
                to_node_id,
                edge_type: persisted_type,
            });
        }
    }

    pub fn log_title_mutation(&mut self, node_key: NodeKey) {
        if let Some(store) = &mut self.services.persistence {
            if let Some(node) = self.workspace.domain.graph.get_node(node_key) {
                store.log_mutation(&LogEntry::UpdateNodeTitle {
                    node_id: node.id.to_string(),
                    title: node.title.clone(),
                });
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
            .map(|payload| payload.has_kind(EdgeKind::TraversalDerived))
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

        self.workspace.history_last_event_unix_ms = Some(Self::unix_timestamp_ms_now());

        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_HISTORY_TRAVERSAL_RECORDED,
            latency_us: 0,
        });

        if !history_semantic_existed {
            self.log_edge_mutation(from_key, to_key, EdgeType::History, None);
        }
        self.log_traversal_mutation(from_key, to_key, traversal);
        self.workspace.physics.base.is_running = true;
        self.workspace.drag_release_frames_remaining = 0;
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
        let already_grouped = self.workspace.domain.graph.edges().any(|edge| {
            edge.edge_type == EdgeType::UserGrouped && edge.from == from && edge.to == to
        });
        if !already_grouped {
            let _ = self.add_edge_and_sync(from, to, EdgeType::UserGrouped, label);
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

        for (&node_key, vector) in &self.workspace.semantic_index {
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
                            edge_type: EdgeType::UserGrouped,
                        },
                        GraphIntent::RemoveEdge {
                            from: to,
                            to: from,
                            edge_type: EdgeType::UserGrouped,
                        },
                    ]
                })
                .unwrap_or_default(),
            EdgeCommand::RemoveUserEdgePair { a, b } => {
                vec![
                    GraphIntent::RemoveEdge {
                        from: a,
                        to: b,
                        edge_type: EdgeType::UserGrouped,
                    },
                    GraphIntent::RemoveEdge {
                        from: b,
                        to: a,
                        edge_type: EdgeType::UserGrouped,
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
            self.workspace.semantic_index_dirty = true;
        }

        if let Some(store) = &mut self.services.persistence {
            store.log_mutation(&LogEntry::PinNode {
                node_id: self
                    .workspace
                    .domain
                    .graph
                    .get_node(key)
                    .map(|node| node.id.to_string())
                    .unwrap_or_default(),
                is_pinned,
            });
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
                    });
                }
            }

            if let Some(webview_id) = self.workspace.node_to_webview.get(&node_key).copied() {
                let _ = self.unmap_webview(webview_id);
            }
            self.remove_active_node(node_key);
            self.remove_warm_cache_node(node_key);
            self.workspace.runtime_block_state.remove(&node_key);
            self.workspace.runtime_block_state.remove(&node_key);
            self.workspace.suggested_semantic_tags.remove(&node_key);
            if let Some(node_id) = node_id {
                self.workspace.node_last_active_workspace.remove(&node_id);
                self.workspace.node_workspace_membership.remove(&node_id);
            }

            if let Some(store) = &mut self.services.persistence {
                let dissolved_before = store.dissolved_archive_len();
                let _ = store.dissolve_and_remove_node(&mut self.workspace.domain.graph, node_key);
                let dissolved_after = store.dissolved_archive_len();
                if dissolved_after > dissolved_before {
                    self.workspace.history_last_event_unix_ms = Some(Self::unix_timestamp_ms_now());
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
        self.workspace.highlighted_graph_edge = None;
        let pending_node_context_target = self
            .pending_node_context_target()
            .filter(|key| self.workspace.domain.graph.get_node(*key).is_some());
        self.set_pending_node_context_target(pending_node_context_target);
        self.sanitize_pending_frame_import_commands();
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
        self.workspace.highlighted_graph_edge = None;
        self.workspace.file_tree_projection_state = FileTreeProjectionState::default();
        self.clear_choose_frame_picker();
        self.workspace.pending_app_commands.clear();
        self.clear_pending_camera_command();
        self.clear_pending_wheel_zoom_delta();
        self.workspace.domain.notes.clear();
        self.workspace.views.clear();
        self.workspace.graph_view_frames.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.webview_to_node.clear();
        self.workspace.node_to_webview.clear();
        self.workspace.active_lru.clear();
        self.workspace.warm_cache_lru.clear();
        self.workspace.runtime_block_state.clear();
        self.workspace.runtime_block_state.clear();
        self.workspace.suggested_semantic_tags.clear();
        self.workspace.semantic_index.clear();
        self.workspace.semantic_index_dirty = true;
        self.workspace.node_last_active_workspace.clear();
        self.workspace.node_workspace_membership.clear();
        self.workspace.last_session_workspace_layout_hash = None;
        self.workspace.last_session_workspace_layout_json = None;
        self.workspace.last_workspace_autosave_at = None;
        self.workspace.current_workspace_is_synthesized = false;
        self.workspace.workspace_has_unsaved_changes = false;
        self.workspace.unsaved_workspace_prompt_warned = false;
        self.workspace.egui_state_dirty = true;
    }

    pub fn clear_graph_and_persistence(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            if let Err(e) = store.clear_all() {
                warn!("Failed to clear persisted graph data: {e}");
            }
        }
        self.workspace.domain.graph = Graph::new();
        self.reset_selection_state();
        self.workspace.highlighted_graph_edge = None;
        self.workspace.file_tree_projection_state = FileTreeProjectionState::default();
        self.clear_choose_frame_picker();
        self.workspace.pending_app_commands.clear();
        self.clear_pending_camera_command();
        self.clear_pending_wheel_zoom_delta();
        self.workspace.views.clear();
        self.workspace.graph_view_frames.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.webview_to_node.clear();
        self.workspace.node_to_webview.clear();
        self.workspace.active_lru.clear();
        self.workspace.warm_cache_lru.clear();
        self.workspace.runtime_block_state.clear();
        self.workspace.runtime_block_state.clear();
        self.workspace.suggested_semantic_tags.clear();
        self.workspace.node_last_active_workspace.clear();
        self.workspace.node_workspace_membership.clear();
        self.workspace.current_workspace_is_synthesized = false;
        self.workspace.workspace_has_unsaved_changes = false;
        self.workspace.unsaved_workspace_prompt_warned = false;
        self.workspace.active_webview_nodes.clear();
        self.workspace.domain.next_placeholder_id = 0;
        self.workspace.egui_state_dirty = true;
        self.workspace.semantic_index.clear();
        self.workspace.semantic_index_dirty = true;
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
                store.log_mutation(&LogEntry::UpdateNodeUrl {
                    node_id: node_id.clone(),
                    new_url: new_url.clone(),
                });
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
        self.workspace.egui_state_dirty = true;
        self.refresh_protocol_probe_for_node(key, &new_url, true);
        Some(old_url)
    }
}
