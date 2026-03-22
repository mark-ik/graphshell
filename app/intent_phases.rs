/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Phase handlers for `apply_reducer_intent_internal`.
//!
//! `apply_reducer_intent_internal` dispatches to these four handlers in order:
//!
//! 1. [`GraphBrowserApp::handle_workspace_view_intent`] — workspace-only view
//!    state (physics, camera, selection, search, projection). Returns `true`
//!    if the intent was consumed.
//! 2. [`GraphBrowserApp::handle_workbench_bridge_intent`] — intents that
//!    forward to the workbench authority via the pending intent queue.  Returns
//!    `true` if consumed.
//! 3. [`GraphBrowserApp::handle_runtime_lifecycle_intent`] — renderer mapping,
//!    node lifecycle, graph-view management, undo/redo, settings, browser
//!    navigation, and cross-cutting runtime intents.  Returns `true` if
//!    consumed.
//! 4. [`GraphBrowserApp::handle_domain_graph_intent`] — canonical graph
//!    structure mutations (create/remove nodes/edges, tagging, history
//!    timeline, etc.).  This handler is the fallthrough; it does not return
//!    a bool.

use super::*;

impl GraphBrowserApp {
    /// Phase 1: workspace-only view state.
    ///
    /// Delegates to the existing `apply_workspace_only_intent` path which
    /// routes through `intent.as_view_action()` → `apply_view_action`.
    /// Returns `true` if the intent was consumed.
    pub(crate) fn handle_workspace_view_intent(&mut self, intent: &GraphIntent) -> bool {
        self.apply_workspace_only_intent(intent)
    }

    /// Phase 2: workbench bridge intents.
    ///
    /// These intents cross into workbench authority territory.  They are
    /// forwarded to the pending workbench intent queue and must not be
    /// processed as graph mutations.  Returns `true` if consumed.
    pub(crate) fn handle_workbench_bridge_intent(&mut self, intent: &GraphIntent) -> bool {
        match intent {
            GraphIntent::SetPanePresentationMode { pane, mode } => {
                self.enqueue_workbench_intent(WorkbenchIntent::SetPanePresentationMode {
                    pane: *pane,
                    mode: *mode,
                });
                true
            }
            GraphIntent::PromoteEphemeralPane {
                target_tile_context,
            } => {
                self.enqueue_workbench_intent(WorkbenchIntent::PromoteEphemeralPane {
                    target_tile_context: target_tile_context.clone(),
                });
                true
            }
            GraphIntent::ToggleHelpPanel => {
                self.enqueue_workbench_intent(WorkbenchIntent::ToggleHelpPanel);
                true
            }
            GraphIntent::ToggleCommandPalette => {
                self.enqueue_workbench_intent(WorkbenchIntent::ToggleCommandPalette);
                true
            }
            GraphIntent::ToggleRadialMenu => {
                self.enqueue_workbench_intent(WorkbenchIntent::ToggleRadialMenu);
                true
            }
            _ => false,
        }
    }

    /// Phase 3: runtime and lifecycle intents.
    ///
    /// Covers renderer mapping, node lifecycle promotion/demotion,
    /// graph-view management, undo/redo, browser navigation, settings
    /// profiles, memory pressure, peer sync, and other cross-cutting
    /// runtime concerns that are not canonical graph structure mutations.
    /// Returns `true` if consumed.
    pub(crate) fn handle_runtime_lifecycle_intent(&mut self, intent: GraphIntent) -> bool {
        match intent {
            GraphIntent::TogglePhysics => {
                self.toggle_physics();
                true
            }
            GraphIntent::TraverseBack => {
                let target = BrowserCommandTarget::ChromeProjection {
                    fallback_node: self.focused_selection().primary(),
                };
                self.request_browser_command(target, BrowserCommand::Back);
                true
            }
            GraphIntent::TraverseForward => {
                let target = BrowserCommandTarget::ChromeProjection {
                    fallback_node: self.focused_selection().primary(),
                };
                self.request_browser_command(target, BrowserCommand::Forward);
                true
            }
            GraphIntent::EnterGraphViewLayoutManager => {
                self.workspace
                    .graph_runtime
                    .graph_view_layout_manager
                    .active = true;
                self.persist_graph_view_layout_manager_state();
                true
            }
            GraphIntent::ExitGraphViewLayoutManager => {
                self.workspace
                    .graph_runtime
                    .graph_view_layout_manager
                    .active = false;
                self.persist_graph_view_layout_manager_state();
                true
            }
            GraphIntent::ToggleGraphViewLayoutManager => {
                self.workspace
                    .graph_runtime
                    .graph_view_layout_manager
                    .active = !self
                    .workspace
                    .graph_runtime
                    .graph_view_layout_manager
                    .active;
                self.persist_graph_view_layout_manager_state();
                true
            }
            GraphIntent::CreateGraphViewSlot {
                anchor_view,
                direction,
                open_mode,
            } => {
                self.create_graph_view_slot(anchor_view, direction, open_mode);
                true
            }
            GraphIntent::RenameGraphViewSlot { view_id, name } => {
                self.rename_graph_view_slot(view_id, name);
                true
            }
            GraphIntent::MoveGraphViewSlot { view_id, row, col } => {
                self.move_graph_view_slot(view_id, row, col);
                true
            }
            GraphIntent::ArchiveGraphViewSlot { view_id } => {
                self.archive_graph_view_slot(view_id);
                true
            }
            GraphIntent::RestoreGraphViewSlot { view_id, row, col } => {
                self.restore_graph_view_slot(view_id, row, col);
                true
            }
            GraphIntent::RouteGraphViewToWorkbench { view_id, mode } => {
                self.route_graph_view_to_workbench(view_id, mode);
                true
            }
            GraphIntent::FocusGraphView { view_id } => {
                self.set_workspace_focused_view_with_transition(Some(view_id));
                true
            }
            GraphIntent::Undo => {
                let current_layout = self.current_undo_checkpoint_layout_json();
                let _ = self.perform_undo(current_layout);
                true
            }
            GraphIntent::Redo => {
                let current_layout = self.current_undo_checkpoint_layout_json();
                let _ = self.perform_redo(current_layout);
                true
            }
            GraphIntent::SetViewLens { view_id, lens } => {
                let requested_layout_algorithm_id = lens.layout_algorithm_id.clone();
                let lens = self.with_registry_lens_defaults(lens);
                let mut lens = if let Some(lens_id) = lens.lens_id.as_deref() {
                    crate::shell::desktop::runtime::registries::phase2_resolve_lens(lens_id)
                } else if lens.name.starts_with("lens:") {
                    crate::shell::desktop::runtime::registries::phase2_resolve_lens(&lens.name)
                } else {
                    lens
                };
                lens.layout_algorithm_id = requested_layout_algorithm_id;
                if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                    view.active_filter = lens.filter_expr.clone();
                    view.lens = lens;
                }
                self.workspace.graph_runtime.egui_state_dirty = true;
                true
            }
            GraphIntent::SetViewFilter { view_id, expr } => {
                let filter_summary = expr.as_ref().map(|expr| {
                    crate::model::graph::filter::evaluate_filter_result(
                        &self.workspace.domain.graph,
                        expr,
                    )
                });
                if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                    let is_some = expr.is_some();
                    view.active_filter = expr.clone();
                    view.lens.filter_expr = expr.clone();
                    let channel = if is_some {
                        crate::shell::desktop::runtime::registries::CHANNEL_UX_FACET_FILTER_APPLIED
                    } else {
                        crate::shell::desktop::runtime::registries::CHANNEL_UX_FACET_FILTER_CLEARED
                    };
                    crate::shell::desktop::runtime::diagnostics::emit_event(
                        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                            channel_id: channel,
                            latency_us: 0,
                        },
                    );
                    if let Some(summary) = filter_summary {
                        for warning in summary.warnings {
                            let channel_id = match warning {
                                crate::model::graph::filter::FilterEvalError::TypeMismatch {
                                    ..
                                } => crate::shell::desktop::runtime::registries::CHANNEL_UX_FACET_FILTER_TYPE_MISMATCH,
                                crate::model::graph::filter::FilterEvalError::InvalidExtensionKey {
                                    ..
                                }
                                | crate::model::graph::filter::FilterEvalError::KeyAbsent {
                                    ..
                                } => crate::shell::desktop::runtime::registries::CHANNEL_UX_FACET_FILTER_EVAL_FAILURE,
                            };
                            crate::shell::desktop::runtime::diagnostics::emit_event(
                                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                                    channel_id,
                                    latency_us: 0,
                                },
                            );
                        }
                    }
                }
                self.workspace.graph_runtime.egui_state_dirty = true;
                true
            }
            GraphIntent::ClearViewFilter { view_id } => {
                if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                    view.active_filter = None;
                    view.lens.filter_expr = None;
                    crate::shell::desktop::runtime::diagnostics::emit_event(
                        crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                            channel_id: crate::shell::desktop::runtime::registries::CHANNEL_UX_FACET_FILTER_CLEARED,
                            latency_us: 0,
                        },
                    );
                }
                self.workspace.graph_runtime.egui_state_dirty = true;
                true
            }
            GraphIntent::SetViewDimension { view_id, dimension } => {
                if !is_semantic_depth_dimension(&dimension) {
                    self.workspace
                        .graph_runtime
                        .semantic_depth_restore_dimensions
                        .remove(&view_id);
                }
                if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                    view.dimension = dimension;
                }
                true
            }
            GraphIntent::ToggleSemanticDepthView { view_id } => {
                let Some(current_dimension) = self
                    .workspace
                    .graph_runtime
                    .views
                    .get(&view_id)
                    .map(|view| view.dimension.clone())
                else {
                    return true;
                };
                let next_dimension = if is_semantic_depth_dimension(&current_dimension) {
                    self.workspace
                        .graph_runtime
                        .semantic_depth_restore_dimensions
                        .remove(&view_id)
                        .unwrap_or_default()
                } else {
                    self.workspace
                        .graph_runtime
                        .semantic_depth_restore_dimensions
                        .insert(view_id, current_dimension);
                    default_semantic_depth_dimension()
                };
                if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                    view.dimension = next_dimension;
                }
                true
            }
            GraphIntent::SetPhysicsProfile { profile_id } => {
                self.set_default_registry_physics_id(Some(&profile_id));
                true
            }
            GraphIntent::SetTheme { theme_id } => {
                self.set_default_registry_theme_id(Some(&theme_id));
                true
            }
            GraphIntent::SetInteracting { interacting } => {
                self.set_interacting(interacting);
                true
            }
            GraphIntent::PromoteNodeToActive { key, cause } => {
                self.promote_node_to_active_with_cause(key, cause);
                true
            }
            GraphIntent::DemoteNodeToWarm { key, cause } => {
                self.demote_node_to_warm_with_cause(key, cause);
                true
            }
            GraphIntent::DemoteNodeToCold { key, cause } => {
                self.demote_node_to_cold_with_cause(key, cause);
                true
            }
            GraphIntent::MarkRuntimeBlocked {
                key,
                reason,
                retry_at,
            } => {
                self.mark_runtime_blocked(key, reason, retry_at);
                true
            }
            GraphIntent::ClearRuntimeBlocked { key, cause } => {
                let _ = cause;
                self.clear_runtime_blocked(key);
                true
            }
            GraphIntent::MapWebviewToNode { webview_id, key } => {
                self.map_webview_to_node(webview_id, key);
                true
            }
            GraphIntent::UnmapWebview { webview_id } => {
                let _ = self.unmap_webview(webview_id);
                true
            }
            GraphIntent::WebViewCreated {
                parent_webview_id,
                child_webview_id,
                initial_url,
            } => {
                self.handle_webview_created(parent_webview_id, child_webview_id, initial_url);
                true
            }
            GraphIntent::WebViewUrlChanged {
                webview_id,
                new_url,
            } => {
                self.handle_webview_url_changed(webview_id, new_url);
                true
            }
            GraphIntent::WebViewHistoryChanged {
                webview_id,
                entries,
                current,
            } => {
                self.handle_webview_history_changed(webview_id, entries, current);
                true
            }
            GraphIntent::WebViewScrollChanged {
                webview_id,
                scroll_x,
                scroll_y,
            } => {
                self.handle_webview_scroll_changed(webview_id, scroll_x, scroll_y);
                true
            }
            GraphIntent::WebViewTitleChanged { webview_id, title } => {
                self.handle_webview_title_changed(webview_id, title);
                true
            }
            GraphIntent::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => {
                self.handle_webview_crashed(webview_id, reason, has_backtrace);
                true
            }
            GraphIntent::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            } => {
                self.set_memory_pressure_status(level, available_mib, total_mib);
                crate::shell::desktop::runtime::registries::phase3_propagate_subsystem_health_memory_pressure(
                    level,
                    available_mib,
                    total_mib,
                );
                true
            }
            GraphIntent::ModActivated { mod_id } => {
                crate::shell::desktop::runtime::registries::phase3_route_mod_lifecycle_event(
                    &mod_id, true,
                );
                log::info!("mod activated: {mod_id}");
                true
            }
            GraphIntent::ModLoadFailed { mod_id, reason } => {
                crate::shell::desktop::runtime::registries::phase3_route_mod_lifecycle_event(
                    &mod_id, false,
                );
                log::warn!("mod load failed: {mod_id} ({reason})");
                true
            }
            GraphIntent::ApplyRemoteDelta { entries } => {
                // TODO: Phase 6.2 - sync integrated logic for applying peer log entries
                log::debug!("peer log entries received: {} bytes", entries.len());
                true
            }
            GraphIntent::SyncNow => {
                match self.request_sync_all_trusted_peers(Self::SESSION_WORKSPACE_LAYOUT_NAME) {
                    Ok(enqueued) => {
                        log::info!("manual Verse sync queued for {} peer(s)", enqueued);
                    }
                    Err(error) => {
                        log::warn!("manual Verse sync unavailable: {error}");
                    }
                }
                true
            }
            GraphIntent::TrustPeer {
                peer_id,
                display_name,
            } => {
                match peer_id.parse::<iroh::NodeId>() {
                    Ok(node_id) => {
                        crate::shell::desktop::runtime::registries::phase3_trust_peer(
                            crate::mods::native::verse::TrustedPeer {
                                node_id,
                                display_name,
                                role: crate::mods::native::verse::PeerRole::Friend,
                                added_at: std::time::SystemTime::now(),
                                last_seen: Some(std::time::SystemTime::now()),
                                workspace_grants: Vec::new(),
                            },
                        );
                        log::info!("paired trusted peer: {peer_id}");
                    }
                    Err(error) => {
                        log::warn!("invalid peer id for trust-peer '{peer_id}': {error}");
                    }
                }
                true
            }
            GraphIntent::GrantWorkspaceAccess {
                peer_id,
                workspace_id,
            } => {
                match peer_id.parse::<iroh::NodeId>() {
                    Ok(node_id) => {
                        crate::shell::desktop::runtime::registries::phase3_grant_workspace_access(
                            node_id,
                            &workspace_id,
                            crate::mods::native::verse::AccessLevel::ReadWrite,
                        );
                        log::info!(
                            "granting workspace access '{}' to peer {}",
                            workspace_id,
                            peer_id
                        );
                    }
                    Err(error) => {
                        log::warn!(
                            "invalid peer id for grant-workspace-access '{peer_id}': {error}"
                        );
                    }
                }
                true
            }
            GraphIntent::ForgetDevice { peer_id } => {
                match peer_id.parse::<iroh::NodeId>() {
                    Ok(node_id) => {
                        crate::shell::desktop::runtime::registries::phase3_revoke_peer(node_id);
                        log::info!("forgetting device: {peer_id}");
                    }
                    Err(error) => {
                        log::warn!("invalid peer id for forget-device '{peer_id}': {error}");
                    }
                }
                true
            }
            GraphIntent::RevokeWorkspaceAccess {
                peer_id,
                workspace_id,
            } => {
                match peer_id.parse::<iroh::NodeId>() {
                    Ok(node_id) => {
                        crate::shell::desktop::runtime::registries::phase3_revoke_workspace_access(
                            node_id,
                            &workspace_id,
                        );
                        log::info!(
                            "revoking workspace access '{}' for peer {}",
                            workspace_id,
                            peer_id
                        );
                    }
                    Err(error) => {
                        log::warn!(
                            "invalid peer id for revoke-workspace-access '{peer_id}': {error}"
                        );
                    }
                }
                true
            }
            GraphIntent::WorkflowActivated { .. } => true,
            GraphIntent::PersistNostrSubscriptions => {
                self.save_persisted_nostr_subscriptions();
                true
            }
            GraphIntent::NostrEventReceived {
                subscription_id,
                event_id,
                pubkey,
                created_at,
                kind,
                content,
                tags,
            } => {
                log::trace!(
                    "nostr event received: sub={subscription_id} kind={kind} id={event_id} from={pubkey} at={created_at} content_len={} tags={}",
                    content.len(),
                    tags.len(),
                );
                true
            }
            GraphIntent::Noop => true,
            GraphIntent::OpenNodeFrameRouted { key, prefer_frame } => {
                self.apply_open_node_frame_routed(key, prefer_frame);
                true
            }
            GraphIntent::OpenNodeWorkspaceRouted {
                key,
                prefer_workspace,
            } => {
                self.apply_open_node_workspace_routed(key, prefer_workspace);
                true
            }
            _ => false,
        }
    }

    /// Phase 4: canonical graph and domain mutations.
    ///
    /// Handles all intents that mutate graph structure: creating/removing
    /// nodes and edges, tagging, history timeline operations, and other
    /// domain-level mutations.  This is the fallthrough handler — it is only
    /// reached when phases 1–3 return `false`.
    pub(crate) fn handle_domain_graph_intent(&mut self, intent: GraphIntent) {
        match intent {
            GraphIntent::CreateNodeNearCenter => {
                self.create_new_node_near_center();
            }
            GraphIntent::CreateNodeNearCenterAndOpen { mode } => {
                // Phase 5: capture the active graphlet context before create_new_node_near_center
                // overwrites the selection with the new node's key.
                let graphlet_peer = if mode == PendingTileOpenMode::Tab {
                    let selected_nodes: Vec<NodeKey> =
                        self.focused_selection().iter().copied().collect();
                    let has_graphlet_context = !selected_nodes.is_empty()
                        && self.graphlet_members_for_active_projection(&selected_nodes).len() > 1;
                    has_graphlet_context
                        .then(|| {
                            self.focused_selection()
                                .primary()
                                .or_else(|| selected_nodes.first().copied())
                        })
                        .flatten()
                } else {
                    None
                };
                let key = self.create_new_node_near_center();
                // Phase 5: when the new tile is opened as a tab into an existing graphlet
                // context, create a durable UserGrouped edge so the new node becomes a
                // permanent graphlet member (survives filter changes).
                if let Some(peer) = graphlet_peer {
                    self.add_user_grouped_edge_if_missing(key, peer, None);
                    self.enqueue_workbench_intent(
                        WorkbenchIntent::ReconcileGraphletTiles { node: key },
                    );
                }
                self.request_open_node_tile_mode(key, mode);
            }
            GraphIntent::CreateNodeAtUrl { url, position } => {
                let key = self.add_node_and_sync(url, position);
                self.select_node(key, false);
            }
            GraphIntent::CreateNodeAtUrlAndOpen {
                url,
                position,
                mode,
            } => {
                let key = self.add_node_and_sync(url, position);
                self.select_node(key, false);
                self.request_open_node_tile_mode(key, mode);
            }
            GraphIntent::AcceptHostOpenRequest { request } => {
                self.handle_host_open_request(request);
            }
            GraphIntent::CreateNoteForNode { key, title } => {
                let _ = self.create_note_for_node(key, title);
            }
            GraphIntent::RemoveSelectedNodes => self.remove_selected_nodes(),
            GraphIntent::ClearGraph => self.clear_graph(),
            GraphIntent::SelectNode { key, multi_select } => {
                self.select_node(key, multi_select);
                // Single-selecting an unloaded node should prewarm it (without opening a tile).
                if !multi_select
                    && self.focused_selection().primary() == Some(key)
                    && !self.is_crash_blocked(key)
                    && self.get_webview_for_node(key).is_none()
                    && self
                        .workspace
                        .domain
                        .graph
                        .get_node(key)
                        .map(|node| node.lifecycle != crate::graph::NodeLifecycle::Active)
                        .unwrap_or(false)
                {
                    self.promote_node_to_active_with_cause(key, LifecycleCause::SelectedPrewarm);
                }
            }
            GraphIntent::CreateUserGroupedEdge { from, to, label } => {
                self.add_user_grouped_edge_if_missing(from, to, label);
                // Queue a one-frame-deferred tile merge: if both nodes already have warm
                // tiles in different containers, ReconcileGraphletTiles consolidates them.
                self.enqueue_workbench_intent(WorkbenchIntent::ReconcileGraphletTiles {
                    node: from,
                });
            }
            GraphIntent::DeleteImportRecord { record_id } => {
                self.delete_import_record(record_id);
            }
            GraphIntent::SuppressImportRecordMembership { record_id, key } => {
                self.suppress_import_record_membership(record_id, key);
            }
            GraphIntent::PromoteImportRecordToUserGroup { record_id, anchor } => {
                self.promote_import_record_to_user_group(record_id, anchor);
            }
            GraphIntent::RemoveEdge {
                from,
                to,
                selector,
            } => {
                if selector == crate::graph::RelationSelector::Family(crate::graph::EdgeFamily::Traversal) {
                    let _ = self.remove_edges_and_log(from, to, crate::graph::EdgeType::History);
                } else {
                    let _ = self.retract_relations_and_log(from, to, selector);
                }
            }
            GraphIntent::CreateUserGroupedEdgeFromPrimarySelection => {
                let primary = self.focused_selection().primary();
                self.create_user_grouped_edge_from_primary_selection();
                if let Some(from) = primary {
                    self.enqueue_workbench_intent(WorkbenchIntent::ReconcileGraphletTiles {
                        node: from,
                    });
                }
            }
            GraphIntent::GroupNodesBySemanticTags => {
                self.group_nodes_by_semantic_tags();
            }
            GraphIntent::ExecuteEdgeCommand { command } => {
                let intents = self.intents_for_edge_command(command);
                self.apply_reducer_intents(intents);
            }
            GraphIntent::SetNodePinned { key, is_pinned } => {
                self.set_node_pinned_and_log(key, is_pinned);
            }
            GraphIntent::TogglePrimaryNodePin => {
                if let Some(key) = self.focused_selection().primary()
                    && let Some(node) = self.workspace.domain.graph.get_node(key)
                {
                    self.apply_reducer_intents([GraphIntent::SetNodePinned {
                        key,
                        is_pinned: !node.is_pinned,
                    }]);
                }
            }
            GraphIntent::SetNodeUrl { key, new_url } => {
                let _ = self.update_node_url_and_log(key, new_url);
            }
            GraphIntent::TagNode { key, tag } => {
                if self.workspace.domain.graph.get_node(key).is_some() {
                    let trimmed = tag.trim();
                    if trimmed.is_empty() {
                        return;
                    }
                    let normalized_tag = if trimmed.starts_with('#') {
                        trimmed.to_ascii_lowercase()
                    } else {
                        match crate::shell::desktop::runtime::registries::phase3_validate_knowledge_tag(
                            trimmed,
                        ) {
                            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Valid {
                                canonical_code, ..
                            } => format!("udc:{canonical_code}"),
                            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Unknown { .. }
                            | crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Malformed { .. } => {
                                trimmed.to_string()
                            }
                        }
                    };
                    if normalized_tag == Self::TAG_PIN {
                        self.set_node_pinned_and_log(key, true);
                    }
                    if self
                        .workspace
                        .domain
                        .graph
                        .insert_node_tag(key, normalized_tag.clone())
                    {
                        self.workspace.graph_runtime.semantic_index_dirty = true;
                    }
                    if let Some(suggestions) = self
                        .workspace
                        .graph_runtime
                        .suggested_semantic_tags
                        .get_mut(&key)
                    {
                        suggestions.retain(|s| s != &normalized_tag);
                        if suggestions.is_empty() {
                            self.workspace
                                .graph_runtime
                                .suggested_semantic_tags
                                .remove(&key);
                        }
                    }
                    // Emit WAL audit event (snapshot entry TagNode has no timestamp).
                    if let Some(store) = &mut self.services.persistence {
                        if let Some(node) = self.workspace.domain.graph.get_node(key) {
                            let node_id = node.id.to_string();
                            store.log_mutation(
                                &crate::services::persistence::types::LogEntry::TagNode {
                                    node_id: node_id.clone(),
                                    tag: normalized_tag.clone(),
                                },
                            );
                            store.log_audit_event(
                                &node_id,
                                crate::services::persistence::types::NodeAuditEventKind::Tagged {
                                    tag: normalized_tag,
                                },
                                Self::unix_timestamp_ms_now(),
                            );
                        }
                    }
                }
            }
            GraphIntent::UntagNode { key, tag } => {
                if tag == Self::TAG_PIN {
                    self.set_node_pinned_and_log(key, false);
                }
                if self.workspace.domain.graph.remove_node_tag(key, &tag) {
                    self.workspace.graph_runtime.semantic_index_dirty = true;
                }
                // Emit WAL audit event (snapshot entry UntagNode has no timestamp).
                if let Some(store) = &mut self.services.persistence {
                    if let Some(node) = self.workspace.domain.graph.get_node(key) {
                        let node_id = node.id.to_string();
                        store.log_mutation(
                            &crate::services::persistence::types::LogEntry::UntagNode {
                                node_id: node_id.clone(),
                                tag: tag.clone(),
                            },
                        );
                        store.log_audit_event(
                            &node_id,
                            crate::services::persistence::types::NodeAuditEventKind::Untagged {
                                tag,
                            },
                            Self::unix_timestamp_ms_now(),
                        );
                    }
                }
            }
            GraphIntent::SuggestNodeTags { key, suggestions } => {
                if self.workspace.domain.graph.get_node(key).is_none() {
                    return;
                }
                let existing_tags = self
                    .workspace
                    .domain
                    .graph
                    .node_tags(key)
                    .cloned()
                    .unwrap_or_default();
                let mut normalized = BTreeSet::new();
                for suggestion in suggestions {
                    match crate::shell::desktop::runtime::registries::phase3_validate_knowledge_tag(
                        &suggestion,
                    ) {
                        crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Valid {
                            canonical_code, ..
                        } => {
                            let canonical = format!("udc:{canonical_code}");
                            if !existing_tags.contains(&canonical) {
                                normalized.insert(canonical);
                            }
                        }
                        crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Unknown { .. }
                        | crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Malformed { .. } => {}
                    }
                }
                if normalized.is_empty() {
                    self.workspace
                        .graph_runtime
                        .suggested_semantic_tags
                        .remove(&key);
                } else {
                    self.workspace
                        .graph_runtime
                        .suggested_semantic_tags
                        .insert(key, normalized.into_iter().collect());
                }
            }
            GraphIntent::UpdateNodeMimeHint { key, mime_hint } => {
                let node_id = self
                    .workspace
                    .domain
                    .graph
                    .get_node(key)
                    .map(|node| node.id);
                let GraphDeltaResult::NodeMetadataUpdated(updated) = self
                    .apply_graph_delta_and_sync(GraphDelta::SetNodeMimeHint {
                        key,
                        mime_hint: mime_hint.clone(),
                    })
                else {
                    unreachable!("mime hint delta must return NodeMetadataUpdated");
                };
                if updated
                    && let Some(store) = &mut self.services.persistence
                    && let Some(node_id) = node_id
                {
                    store.log_mutation(&LogEntry::UpdateNodeMimeHint {
                        node_id: node_id.to_string(),
                        mime_hint,
                    });
                }
                if updated && let Some(node) = self.workspace.domain.graph.get_node(key) {
                    crate::shell::desktop::runtime::registries::phase3_publish_navigation_mime_resolved(
                        key,
                        &node.url,
                        node.mime_hint.as_deref(),
                    );
                }
            }
            GraphIntent::UpdateNodeAddressKind { key, kind } => {
                let node_id = self
                    .workspace
                    .domain
                    .graph
                    .get_node(key)
                    .map(|node| node.id);
                let GraphDeltaResult::NodeMetadataUpdated(updated) =
                    self.apply_graph_delta_and_sync(GraphDelta::SetNodeAddressKind { key, kind })
                else {
                    unreachable!("address kind delta must return NodeMetadataUpdated");
                };
                if updated
                    && let Some(store) = &mut self.services.persistence
                    && let Some(node_id) = node_id
                {
                    let persisted_kind = match kind {
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
                        node_id: node_id.to_string(),
                        kind: persisted_kind,
                    });
                }
            }
            GraphIntent::ClearHistoryTimeline
            | GraphIntent::ClearHistoryDissolved
            | GraphIntent::AutoCurateHistoryTimeline { .. }
            | GraphIntent::AutoCurateHistoryDissolved { .. }
            | GraphIntent::ExportHistoryTimeline
            | GraphIntent::ExportHistoryDissolved
            | GraphIntent::EnterHistoryTimelinePreview
            | GraphIntent::ExitHistoryTimelinePreview
            | GraphIntent::HistoryTimelinePreviewIsolationViolation { .. }
            | GraphIntent::HistoryTimelineReplayStarted
            | GraphIntent::HistoryTimelineReplaySetTotal { .. }
            | GraphIntent::HistoryTimelineReplayAdvance { .. }
            | GraphIntent::HistoryTimelineReplayReset
            | GraphIntent::HistoryTimelineReplayProgress { .. }
            | GraphIntent::HistoryTimelineReplayFinished { .. }
            | GraphIntent::HistoryTimelineReturnToPresentFailed { .. } => {
                self.apply_history_runtime_intent(intent)
            }
            // Workspace-only intents are handled in phase 1 via apply_workspace_only_intent.
            // This arm is unreachable at runtime but makes the match exhaustive.
            GraphIntent::ToggleCameraPositionFitLock
            | GraphIntent::ToggleCameraZoomFitLock
            | GraphIntent::RequestFitToScreen
            | GraphIntent::RequestZoomIn
            | GraphIntent::RequestZoomOut
            | GraphIntent::RequestZoomReset
            | GraphIntent::RequestZoomToSelected
            | GraphIntent::ReheatPhysics
            | GraphIntent::UpdateSelection { .. }
            | GraphIntent::SelectAll
            | GraphIntent::SetNodePosition { .. }
            | GraphIntent::SetZoom { .. }
            | GraphIntent::SetHighlightedEdge { .. }
            | GraphIntent::ClearHighlightedEdge
            | GraphIntent::SetNodeFormDraft { .. }
            | GraphIntent::SetNodeThumbnail { .. }
            | GraphIntent::SetNodeFavicon { .. }
            | GraphIntent::SetWorkbenchEdgeProjection { .. }
            | GraphIntent::SetViewEdgeProjectionOverride { .. }
            | GraphIntent::SetSelectionEdgeProjectionOverride { .. }
            | GraphIntent::SetNavigatorContainmentRelationSource { .. }
            | GraphIntent::SetNavigatorSortMode { .. }
            | GraphIntent::SetNavigatorRootFilter { .. }
            | GraphIntent::SetNavigatorSelectedRows { .. }
            | GraphIntent::SetNavigatorExpandedRows { .. }
            | GraphIntent::RebuildNavigatorProjection => {
                unreachable!("workspace-only intents are handled in phase 1");
            }
            // Workbench bridge intents are handled in phase 2.
            GraphIntent::SetPanePresentationMode { .. }
            | GraphIntent::PromoteEphemeralPane { .. }
            | GraphIntent::ToggleHelpPanel
            | GraphIntent::ToggleCommandPalette
            | GraphIntent::ToggleRadialMenu => {
                unreachable!("workbench bridge intents are handled in phase 2")
            }
            // Runtime lifecycle intents are handled in phase 3.
            GraphIntent::TogglePhysics
            | GraphIntent::TraverseBack
            | GraphIntent::TraverseForward
            | GraphIntent::EnterGraphViewLayoutManager
            | GraphIntent::ExitGraphViewLayoutManager
            | GraphIntent::ToggleGraphViewLayoutManager
            | GraphIntent::CreateGraphViewSlot { .. }
            | GraphIntent::RenameGraphViewSlot { .. }
            | GraphIntent::MoveGraphViewSlot { .. }
            | GraphIntent::ArchiveGraphViewSlot { .. }
            | GraphIntent::RestoreGraphViewSlot { .. }
            | GraphIntent::RouteGraphViewToWorkbench { .. }
            | GraphIntent::Undo
            | GraphIntent::Redo
            | GraphIntent::SetViewLens { .. }
            | GraphIntent::SetViewFilter { .. }
            | GraphIntent::ClearViewFilter { .. }
            | GraphIntent::SetViewDimension { .. }
            | GraphIntent::ToggleSemanticDepthView { .. }
            | GraphIntent::SetPhysicsProfile { .. }
            | GraphIntent::SetTheme { .. }
            | GraphIntent::SetInteracting { .. }
            | GraphIntent::PromoteNodeToActive { .. }
            | GraphIntent::DemoteNodeToWarm { .. }
            | GraphIntent::DemoteNodeToCold { .. }
            | GraphIntent::MarkRuntimeBlocked { .. }
            | GraphIntent::ClearRuntimeBlocked { .. }
            | GraphIntent::MapWebviewToNode { .. }
            | GraphIntent::UnmapWebview { .. }
            | GraphIntent::WebViewCreated { .. }
            | GraphIntent::WebViewUrlChanged { .. }
            | GraphIntent::WebViewHistoryChanged { .. }
            | GraphIntent::WebViewScrollChanged { .. }
            | GraphIntent::WebViewTitleChanged { .. }
            | GraphIntent::WebViewCrashed { .. }
            | GraphIntent::SetMemoryPressureStatus { .. }
            | GraphIntent::ModActivated { .. }
            | GraphIntent::ModLoadFailed { .. }
            | GraphIntent::ApplyRemoteDelta { .. }
            | GraphIntent::SyncNow
            | GraphIntent::TrustPeer { .. }
            | GraphIntent::GrantWorkspaceAccess { .. }
            | GraphIntent::ForgetDevice { .. }
            | GraphIntent::RevokeWorkspaceAccess { .. }
            | GraphIntent::WorkflowActivated { .. }
            | GraphIntent::PersistNostrSubscriptions
            | GraphIntent::NostrEventReceived { .. }
            | GraphIntent::Noop
            | GraphIntent::OpenNodeFrameRouted { .. }
            | GraphIntent::OpenNodeWorkspaceRouted { .. }
            | GraphIntent::FocusGraphView { .. } => {
                unreachable!("runtime lifecycle intents are handled in phase 3")
            }
        }
    }
}
