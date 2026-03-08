use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use crate::app::GraphViewId;
use crate::graph::NodeKey;

/// Canonical node-selection state.
///
/// This wraps the selected-node set with explicit metadata so consumers can
/// reason about selection changes deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionState {
    nodes: HashSet<NodeKey>,
    order: Vec<NodeKey>,
    primary: Option<NodeKey>,
    revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionUpdateMode {
    Replace,
    Add,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardCopyKind {
    Url,
    Title,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardCopyRequest {
    pub key: NodeKey,
    pub kind: ClipboardCopyKind,
}

#[derive(Clone)]
pub(crate) struct UndoRedoSnapshot {
    pub(crate) graph_bytes: Vec<u8>,
    pub(crate) selected_nodes: SelectionState,
    pub(crate) selected_nodes_by_view: HashMap<GraphViewId, SelectionState>,
    pub(crate) highlighted_graph_edge: Option<(NodeKey, NodeKey)>,
    pub(crate) workspace_layout_json: Option<String>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Monotonic revision incremented whenever the selection changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Primary selected node (most recently selected).
    pub fn primary(&self) -> Option<NodeKey> {
        self.primary
    }

    pub fn select(&mut self, key: NodeKey, multi_select: bool) {
        if multi_select {
            if self.nodes.contains(&key) {
                self.nodes.remove(&key);
                self.order.retain(|existing| *existing != key);
                self.primary = self.order.last().copied();
                self.revision = self.revision.saturating_add(1);
            } else if self.nodes.insert(key) {
                self.order.push(key);
                self.primary = Some(key);
                self.revision = self.revision.saturating_add(1);
            }
            return;
        }

        if self.nodes.len() == 1 && self.nodes.contains(&key) && self.primary == Some(key) {
            self.nodes.clear();
            self.order.clear();
            self.primary = None;
            self.revision = self.revision.saturating_add(1);
            return;
        }

        self.nodes.clear();
        self.order.clear();
        self.nodes.insert(key);
        self.order.push(key);
        self.primary = Some(key);
        self.revision = self.revision.saturating_add(1);
    }

    pub fn clear(&mut self) {
        if self.nodes.is_empty() && self.primary.is_none() {
            return;
        }
        self.nodes.clear();
        self.order.clear();
        self.primary = None;
        self.revision = self.revision.saturating_add(1);
    }

    pub fn update_many(&mut self, keys: Vec<NodeKey>, mode: SelectionUpdateMode) {
        match mode {
            SelectionUpdateMode::Replace => {
                self.nodes.clear();
                self.order.clear();
                for key in keys {
                    if self.nodes.insert(key) {
                        self.order.push(key);
                    }
                }
                self.primary = self.order.last().copied();
                self.revision = self.revision.saturating_add(1);
            }
            SelectionUpdateMode::Add => {
                let mut changed = false;
                for key in keys {
                    if self.nodes.insert(key) {
                        self.order.push(key);
                        self.primary = Some(key);
                        changed = true;
                    }
                }
                if changed {
                    self.revision = self.revision.saturating_add(1);
                }
            }
            SelectionUpdateMode::Toggle => {
                let mut changed = false;
                for key in keys {
                    if self.nodes.remove(&key) {
                        self.order.retain(|existing| *existing != key);
                        changed = true;
                    } else if self.nodes.insert(key) {
                        self.order.push(key);
                        self.primary = Some(key);
                        changed = true;
                    }
                }
                self.primary = self.order.last().copied();
                if changed {
                    self.revision = self.revision.saturating_add(1);
                }
            }
        }
    }

    pub fn retain_nodes<F>(&mut self, mut keep: F)
    where
        F: FnMut(NodeKey) -> bool,
    {
        let had_primary = self.primary;
        let previous_len = self.nodes.len();

        self.nodes.retain(|key| keep(*key));
        self.order.retain(|key| self.nodes.contains(key));
        self.primary = self.order.last().copied();

        if previous_len != self.nodes.len() || had_primary != self.primary {
            self.revision = self.revision.saturating_add(1);
        }
    }

    /// Ordered pair of selected nodes when exactly two nodes are selected.
    pub fn ordered_pair(&self) -> Option<(NodeKey, NodeKey)> {
        if self.nodes.len() != 2 {
            return None;
        }
        let mut iter = self
            .order
            .iter()
            .copied()
            .filter(|key| self.nodes.contains(key));
        let first = iter.next()?;
        let second = iter.next()?;
        Some((first, second))
    }
}

impl Deref for SelectionState {
    type Target = HashSet<NodeKey>;

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}
