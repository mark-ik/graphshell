use super::*;

impl GraphBrowserApp {
    pub(crate) fn handle_webview_created(
        &mut self,
        parent_webview_id: RendererId,
        child_webview_id: RendererId,
        initial_url: Option<String>,
    ) {
        let parent_node = self.get_node_for_webview(parent_webview_id);
        let position = if let Some(parent_key) = parent_node {
            use rand::Rng;

            let mut rng = rand::thread_rng();
            let jitter_x = rng.gen_range(-50.0_f32..50.0_f32);
            let jitter_y = rng.gen_range(-50.0_f32..50.0_f32);
            self.workspace
                .domain
                .graph
                .node_projected_position(parent_key)
                .map(|position| {
                    Point2D::new(
                        position.x + 140.0 + jitter_x,
                        position.y + 80.0 + jitter_y,
                    )
                })
                .unwrap_or_else(|| Point2D::new(400.0, 300.0))
        } else {
            Point2D::new(400.0, 300.0)
        };
        let node_url = initial_url
            .filter(|url| !url.is_empty() && url != "about:blank")
            .unwrap_or_else(|| self.next_placeholder_url());
        let child_node = self.add_node_and_sync(node_url, position);
        self.apply_runtime_events([
            RuntimeEvent::MapWebviewToNode {
                webview_id: child_webview_id,
                key: child_node,
            },
            RuntimeEvent::PromoteNodeToActive {
                key: child_node,
                cause: LifecycleCause::Restore,
            },
        ]);
        if let Some(parent_key) = parent_node {
            let _ = self.add_edge_and_sync(parent_key, child_node, EdgeType::Hyperlink);
        }
    }

    pub(crate) fn handle_webview_url_changed(&mut self, webview_id: RendererId, new_url: String) {
        if new_url.is_empty() {
            return;
        }
        let Some(node_key) = self.get_node_for_webview(webview_id) else {
            return;
        };
        let _ = self
            .workspace
            .domain
            .graph
            .touch_node_last_visited_now(node_key);
        if self
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .map(|node| node.url != new_url)
            .unwrap_or(false)
        {
            let to_key = self
                .workspace
                .domain
                .graph
                .get_node_by_url(&new_url)
                .map(|(key, _)| key);
            if let Some(to_key) = to_key {
                self.push_history_traversal_and_sync(node_key, to_key, NavigationTrigger::Unknown);
            }
            let _ = self.update_node_url_and_log(node_key, new_url);
        }
    }

    pub(crate) fn handle_webview_history_changed(
        &mut self,
        webview_id: RendererId,
        entries: Vec<String>,
        current: usize,
    ) {
        let Some(node_key) = self.get_node_for_webview(webview_id) else {
            return;
        };
        let (old_entries, old_index) =
            if let Some(node) = self.workspace.domain.graph.get_node(node_key) {
                (node.history_entries.clone(), node.history_index)
            } else {
                return;
            };
        let new_index = if entries.is_empty() {
            0
        } else {
            current.min(entries.len() - 1)
        };
        self.maybe_add_history_traversal_edge(
            node_key,
            &old_entries,
            old_index,
            &entries,
            new_index,
        );
        let _ = self
            .workspace
            .domain
            .graph
            .set_node_history_state(node_key, entries, new_index);
    }

    pub(crate) fn handle_webview_scroll_changed(
        &mut self,
        webview_id: RendererId,
        scroll_x: f32,
        scroll_y: f32,
    ) {
        let Some(node_key) = self.get_node_for_webview(webview_id) else {
            return;
        };
        let _ = self
            .workspace
            .domain
            .graph
            .set_node_session_scroll(node_key, Some((scroll_x, scroll_y)));
    }

    pub(crate) fn handle_webview_title_changed(
        &mut self,
        webview_id: RendererId,
        title: Option<String>,
    ) {
        let Some(node_key) = self.get_node_for_webview(webview_id) else {
            return;
        };
        let Some(title) = title else {
            return;
        };
        if title.is_empty() {
            return;
        }
        let GraphDeltaResult::NodeMetadataUpdated(changed) = self
            .apply_graph_delta_and_sync(GraphDelta::SetNodeTitle {
                key: node_key,
                title,
            })
        else {
            unreachable!("title delta must return NodeMetadataUpdated");
        };
        if changed {
            self.log_title_mutation(node_key);
        }
    }

    pub(crate) fn handle_webview_crashed(
        &mut self,
        webview_id: RendererId,
        reason: String,
        has_backtrace: bool,
    ) {
        if let Some(node_key) = self.get_node_for_webview(webview_id) {
            self.mark_runtime_crash_blocked(node_key, reason.clone(), has_backtrace);
            self.apply_runtime_events([RuntimeEvent::DemoteNodeToCold {
                key: node_key,
                cause: LifecycleCause::Crash,
            }]);
        } else {
            let _ = self.unmap_webview(webview_id);
        }
        warn!(
            "WebView {:?} crashed: reason={} has_backtrace={}",
            webview_id, reason, has_backtrace
        );
    }

    pub fn map_webview_to_node(&mut self, webview_id: RendererId, node_key: NodeKey) {
        if let Some(previous_node) = self.workspace.webview_to_node.remove(&webview_id) {
            self.workspace.node_to_webview.remove(&previous_node);
            self.remove_active_node(previous_node);
            self.remove_warm_cache_node(previous_node);
        }
        if let Some(previous_webview_id) = self.workspace.node_to_webview.remove(&node_key) {
            self.workspace.webview_to_node.remove(&previous_webview_id);
        }
        self.workspace.webview_to_node.insert(webview_id, node_key);
        self.workspace.node_to_webview.insert(node_key, webview_id);
        self.touch_active_node(node_key);
        self.remove_warm_cache_node(node_key);
    }

    pub fn unmap_webview(&mut self, webview_id: RendererId) -> Option<NodeKey> {
        if let Some(node_key) = self.workspace.webview_to_node.remove(&webview_id) {
            self.workspace.node_to_webview.remove(&node_key);
            self.remove_active_node(node_key);
            self.remove_warm_cache_node(node_key);
            Some(node_key)
        } else {
            None
        }
    }

    pub fn get_node_for_webview(&self, webview_id: RendererId) -> Option<NodeKey> {
        self.workspace.webview_to_node.get(&webview_id).copied()
    }

    pub fn runtime_block_state_for_node(&self, node_key: NodeKey) -> Option<&RuntimeBlockState> {
        self.workspace.runtime_block_state.get(&node_key)
    }

    pub fn mark_runtime_blocked(
        &mut self,
        node_key: NodeKey,
        reason: RuntimeBlockReason,
        retry_at: Option<Instant>,
    ) {
        if self.workspace.domain.graph.get_node(node_key).is_none() {
            self.workspace.runtime_block_state.remove(&node_key);
            return;
        }
        self.workspace.runtime_block_state.insert(
            node_key,
            RuntimeBlockState {
                reason,
                retry_at,
                message: None,
                has_backtrace: false,
                blocked_at: SystemTime::now(),
            },
        );
    }

    pub fn clear_runtime_blocked(&mut self, node_key: NodeKey) {
        self.workspace.runtime_block_state.remove(&node_key);
    }

    pub fn mark_runtime_crash_blocked(
        &mut self,
        node_key: NodeKey,
        message: String,
        has_backtrace: bool,
    ) {
        if self.workspace.domain.graph.get_node(node_key).is_none() {
            self.workspace.runtime_block_state.remove(&node_key);
            return;
        }
        self.workspace.runtime_block_state.insert(
            node_key,
            RuntimeBlockState {
                reason: RuntimeBlockReason::Crash,
                retry_at: None,
                message: Some(message),
                has_backtrace,
                blocked_at: SystemTime::now(),
            },
        );
    }

    pub fn runtime_crash_state_for_node(&self, node_key: NodeKey) -> Option<&RuntimeBlockState> {
        self.workspace
            .runtime_block_state
            .get(&node_key)
            .filter(|state| state.reason == RuntimeBlockReason::Crash)
    }

    pub fn crash_blocked_node_keys(&self) -> impl Iterator<Item = NodeKey> + '_ {
        self.workspace
            .runtime_block_state
            .iter()
            .filter_map(|(key, state)| (state.reason == RuntimeBlockReason::Crash).then_some(*key))
    }

    pub fn is_crash_blocked(&self, node_key: NodeKey) -> bool {
        self.runtime_crash_state_for_node(node_key).is_some()
    }

    pub fn is_runtime_blocked(&mut self, node_key: NodeKey, now: Instant) -> bool {
        let Some(state) = self.workspace.runtime_block_state.get(&node_key) else {
            return false;
        };
        if let Some(retry_at) = state.retry_at
            && retry_at <= now
        {
            self.workspace.runtime_block_state.remove(&node_key);
            return false;
        }
        true
    }

    pub fn get_webview_for_node(&self, node_key: NodeKey) -> Option<RendererId> {
        self.workspace.node_to_webview.get(&node_key).copied()
    }

    pub fn webview_node_mappings(&self) -> impl Iterator<Item = (RendererId, NodeKey)> + '_ {
        self.workspace
            .webview_to_node
            .iter()
            .map(|(&webview_id, &node_key)| (webview_id, node_key))
    }

    #[allow(dead_code)]
    pub fn promote_node_to_active(&mut self, node_key: NodeKey) {
        self.promote_node_to_active_with_cause(node_key, LifecycleCause::Restore);
    }

    pub fn promote_node_to_active_with_cause(&mut self, node_key: NodeKey, cause: LifecycleCause) {
        use crate::graph::NodeLifecycle;

        if self.workspace.domain.graph.get_node(node_key).is_none() {
            return;
        }

        let is_crashed = self.is_crash_blocked(node_key);
        if is_crashed && !matches!(cause, LifecycleCause::UserSelect | LifecycleCause::Restore) {
            return;
        }

        let _ = self
            .workspace
            .domain
            .graph
            .set_node_lifecycle(node_key, NodeLifecycle::Active);
        self.touch_active_node(node_key);
        self.remove_warm_cache_node(node_key);
        self.workspace.runtime_block_state.remove(&node_key);
        if matches!(cause, LifecycleCause::UserSelect | LifecycleCause::Restore) {
            self.workspace.runtime_block_state.remove(&node_key);
        }
    }

    #[allow(dead_code)]
    pub fn demote_node_to_warm(&mut self, node_key: NodeKey) {
        self.demote_node_to_warm_with_cause(node_key, LifecycleCause::WorkspaceRetention);
    }

    pub fn demote_node_to_warm_with_cause(&mut self, node_key: NodeKey, cause: LifecycleCause) {
        use crate::graph::NodeLifecycle;

        if self.workspace.domain.graph.get_node(node_key).is_none() {
            return;
        }

        if matches!(
            cause,
            LifecycleCause::Crash
                | LifecycleCause::ExplicitClose
                | LifecycleCause::NodeRemoval
                | LifecycleCause::MemoryPressureCritical
        ) {
            self.demote_node_to_cold_with_cause(node_key, cause);
            return;
        }

        let has_mapped_webview = self.workspace.node_to_webview.contains_key(&node_key);
        let _ = self
            .workspace
            .domain
            .graph
            .set_node_lifecycle(node_key, NodeLifecycle::Warm);
        if has_mapped_webview {
            self.touch_warm_cache_node(node_key);
        } else {
            self.remove_warm_cache_node(node_key);
        }
        self.remove_active_node(node_key);
    }

    #[allow(dead_code)]
    pub fn demote_node_to_cold(&mut self, node_key: NodeKey) {
        self.demote_node_to_cold_with_cause(node_key, LifecycleCause::NodeRemoval);
    }

    pub fn demote_node_to_cold_with_cause(&mut self, node_key: NodeKey, cause: LifecycleCause) {
        use crate::graph::NodeLifecycle;

        if self.workspace.domain.graph.get_node(node_key).is_none() {
            return;
        }
        let _ = self
            .workspace
            .domain
            .graph
            .set_node_lifecycle(node_key, NodeLifecycle::Cold);
        self.remove_active_node(node_key);
        self.remove_warm_cache_node(node_key);
        if !matches!(cause, LifecycleCause::Crash) {
            self.workspace.runtime_block_state.remove(&node_key);
        }
        if let Some(webview_id) = self.workspace.node_to_webview.get(&node_key).copied() {
            self.workspace.webview_to_node.remove(&webview_id);
            self.workspace.node_to_webview.remove(&node_key);
        }
        if !matches!(cause, LifecycleCause::Crash) {
            self.workspace.runtime_block_state.remove(&node_key);
        }
    }

    fn touch_active_node(&mut self, node_key: NodeKey) {
        self.remove_active_node(node_key);
        self.workspace.active_lru.push(node_key);
    }

    pub(crate) fn remove_active_node(&mut self, node_key: NodeKey) {
        self.workspace.active_lru.retain(|key| *key != node_key);
    }

    fn touch_warm_cache_node(&mut self, node_key: NodeKey) {
        self.remove_warm_cache_node(node_key);
        self.workspace.warm_cache_lru.push(node_key);
    }

    pub(crate) fn remove_warm_cache_node(&mut self, node_key: NodeKey) {
        self.workspace.warm_cache_lru.retain(|key| *key != node_key);
    }

    pub(crate) fn take_warm_cache_evictions(&mut self) -> Vec<NodeKey> {
        let mut normalized = Vec::with_capacity(self.workspace.warm_cache_lru.len());
        let drained: Vec<_> = self.workspace.warm_cache_lru.drain(..).collect();
        for key in drained {
            let keep = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.lifecycle == crate::graph::NodeLifecycle::Warm)
                .unwrap_or(false)
                && self.workspace.node_to_webview.contains_key(&key)
                && !normalized.contains(&key);
            if keep {
                normalized.push(key);
            }
        }
        self.workspace.warm_cache_lru = normalized;

        let mut evicted = Vec::new();
        while self.workspace.warm_cache_lru.len() > self.workspace.warm_cache_limit {
            evicted.push(self.workspace.warm_cache_lru.remove(0));
        }
        evicted
    }

    pub(crate) fn take_active_webview_evictions(
        &mut self,
        protected: &HashSet<NodeKey>,
    ) -> Vec<NodeKey> {
        self.take_active_webview_evictions_with_limit(
            self.workspace.active_webview_limit,
            protected,
        )
    }

    pub(crate) fn take_active_webview_evictions_with_limit(
        &mut self,
        limit: usize,
        protected: &HashSet<NodeKey>,
    ) -> Vec<NodeKey> {
        let mut normalized = Vec::with_capacity(self.workspace.active_lru.len());
        let drained: Vec<_> = self.workspace.active_lru.drain(..).collect();
        for key in drained {
            let keep = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.lifecycle == crate::graph::NodeLifecycle::Active)
                .unwrap_or(false)
                && self.workspace.node_to_webview.contains_key(&key)
                && !normalized.contains(&key);
            if keep {
                normalized.push(key);
            }
        }

        for (&key, _) in &self.workspace.node_to_webview {
            let is_active = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.lifecycle == crate::graph::NodeLifecycle::Active)
                .unwrap_or(false);
            if is_active && !normalized.contains(&key) {
                normalized.push(key);
            }
        }
        self.workspace.active_lru = normalized;

        let mut evicted = Vec::new();
        while self.workspace.active_lru.len() > limit {
            let candidate_idx = self
                .workspace
                .active_lru
                .iter()
                .position(|key| !protected.contains(key));
            let Some(candidate_idx) = candidate_idx else {
                break;
            };
            let key = self.workspace.active_lru.remove(candidate_idx);
            evicted.push(key);
        }
        evicted
    }
}
