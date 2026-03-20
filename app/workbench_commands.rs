use super::*;
use egui_tiles::{Tile, TileId, Tree};

use crate::shell::desktop::workbench::tile_kind::TileKind;

use super::arrangement_graph_bridge::{ArrangementGraphDelta, ArrangementSnapshot};

impl GraphBrowserApp {
    pub fn enqueue_workbench_intent(&mut self, intent: WorkbenchIntent) {
        self.workspace
            .workbench_session
            .pending_workbench_intents
            .push(intent);
    }

    pub fn extend_workbench_intents<I>(&mut self, intents: I)
    where
        I: IntoIterator<Item = WorkbenchIntent>,
    {
        self.workspace
            .workbench_session
            .pending_workbench_intents
            .extend(intents);
    }

    pub fn take_pending_workbench_intents(&mut self) -> Vec<WorkbenchIntent> {
        std::mem::take(&mut self.workspace.workbench_session.pending_workbench_intents)
    }

    #[cfg(test)]
    pub fn pending_workbench_intent_count_for_tests(&self) -> usize {
        self.workspace
            .workbench_session
            .pending_workbench_intents
            .len()
    }

    pub fn workbench_tile_selection(&self) -> &WorkbenchTileSelectionState {
        &self.workbench_tile_selection
    }

    pub fn clear_workbench_tile_selection(&mut self) {
        self.workbench_tile_selection.selected_tile_ids.clear();
        self.workbench_tile_selection.primary_tile_id = None;
    }

    pub fn select_workbench_tile(&mut self, tile_id: TileId) {
        self.update_workbench_tile_selection(tile_id, SelectionUpdateMode::Replace);
    }

    pub fn update_workbench_tile_selection(&mut self, tile_id: TileId, mode: SelectionUpdateMode) {
        match mode {
            SelectionUpdateMode::Replace => {
                self.workbench_tile_selection.selected_tile_ids.clear();
                self.workbench_tile_selection
                    .selected_tile_ids
                    .insert(tile_id);
                self.workbench_tile_selection.primary_tile_id = Some(tile_id);
            }
            SelectionUpdateMode::Add => {
                self.workbench_tile_selection
                    .selected_tile_ids
                    .insert(tile_id);
                self.workbench_tile_selection.primary_tile_id = Some(tile_id);
            }
            SelectionUpdateMode::Toggle => {
                if self
                    .workbench_tile_selection
                    .selected_tile_ids
                    .remove(&tile_id)
                {
                    if self.workbench_tile_selection.primary_tile_id == Some(tile_id) {
                        self.workbench_tile_selection.primary_tile_id = self
                            .workbench_tile_selection
                            .selected_tile_ids
                            .iter()
                            .copied()
                            .next();
                    }
                } else {
                    self.workbench_tile_selection
                        .selected_tile_ids
                        .insert(tile_id);
                    self.workbench_tile_selection.primary_tile_id = Some(tile_id);
                }
            }
        }
    }

    pub fn prune_workbench_tile_selection(&mut self, tiles_tree: &Tree<TileKind>) {
        self.workbench_tile_selection
            .selected_tile_ids
            .retain(|tile_id| matches!(tiles_tree.tiles.get(*tile_id), Some(Tile::Pane(_))));
        if self
            .workbench_tile_selection
            .primary_tile_id
            .is_some_and(|tile_id| {
                !self
                    .workbench_tile_selection
                    .selected_tile_ids
                    .contains(&tile_id)
            })
        {
            self.workbench_tile_selection.primary_tile_id = self
                .workbench_tile_selection
                .selected_tile_ids
                .iter()
                .copied()
                .next();
        }
    }

    /// Persist a tile-group arrangement for the given selection.
    ///
    /// Extracts pane tile-kinds from the tree and selection, then delegates
    /// to the arrangement→graph bridge via [`ArrangementSnapshot::TileGroup`].
    pub(crate) fn persist_workbench_tile_group(
        &mut self,
        tiles_tree: &Tree<TileKind>,
        selected_tile_ids: &std::collections::HashSet<TileId>,
    ) -> Option<NodeKey> {
        let pane_tile_kinds: Vec<TileKind> = selected_tile_ids
            .iter()
            .filter_map(|tile_id| match tiles_tree.tiles.get(*tile_id) {
                Some(Tile::Pane(kind)) => Some(kind.clone()),
                _ => None,
            })
            .collect();

        let snapshot = ArrangementSnapshot::TileGroup { pane_tile_kinds };
        let delta = self.apply_arrangement_snapshot(&snapshot);
        delta.container_node
    }

    /// Sync the graph representation of a named workbench frame.
    ///
    /// Extracts all pane tile-kinds from the tree, then delegates to the
    /// arrangement→graph bridge via [`ArrangementSnapshot::Frame`].
    pub(crate) fn sync_named_workbench_frame_graph_representation(
        &mut self,
        name: &str,
        tiles_tree: &Tree<TileKind>,
    ) -> NodeKey {
        let pane_tile_kinds: Vec<TileKind> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(kind.clone()),
                _ => None,
            })
            .collect();

        let snapshot = ArrangementSnapshot::Frame {
            name: name.to_string(),
            pane_tile_kinds,
        };
        let delta = self.apply_arrangement_snapshot(&snapshot);
        // Frame snapshots always produce a container node.
        delta
            .container_node
            .expect("frame snapshot must produce a container node")
    }

    /// Remove the graph representation of a named workbench frame.
    ///
    /// Delegates to the arrangement→graph bridge via
    /// [`ArrangementSnapshot::RemoveFrame`].
    pub(crate) fn remove_named_workbench_frame_graph_representation(&mut self, name: &str) {
        let snapshot = ArrangementSnapshot::RemoveFrame {
            name: name.to_string(),
        };
        let _ = self.apply_arrangement_snapshot(&snapshot);
    }
}
