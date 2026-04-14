use std::collections::{HashMap, VecDeque};

use crate::graph::{Graph, NodeKey};
use crate::registries::atomic::lens::LayoutMode;
use euclid::default::Point2D;
use petgraph::Direction;

pub(crate) const GRAPH_LAYOUT_FORCE_DIRECTED: &str = "graph_layout:force_directed";
pub(crate) const GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT: &str =
    "graph_layout:force_directed_barnes_hut";
pub(crate) const GRAPH_LAYOUT_GRID: &str = "graph_layout:grid";
pub(crate) const GRAPH_LAYOUT_TREE: &str = "graph_layout:tree";
const LEGACY_LAYOUT_DEFAULT: &str = "layout:default";
const LEGACY_LAYOUT_GRID: &str = "layout:grid";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutCapability {
    pub(crate) algorithm_id: String,
    pub(crate) display_name: String,
    pub(crate) deterministic: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) layout_mode: LayoutMode,
    pub(crate) capability: LayoutCapability,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutExecution {
    pub(crate) resolution: LayoutResolution,
    pub(crate) changed_positions: usize,
    pub(crate) stable: bool,
}

pub(crate) trait LayoutAlgorithm: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_deterministic(&self) -> bool;
    fn execute(
        &self,
        graph: &mut Graph,
        layout_mode: &LayoutMode,
    ) -> Result<LayoutExecution, String>;
}

pub(crate) struct ForceDirectedLayout;

impl LayoutAlgorithm for ForceDirectedLayout {
    fn id(&self) -> &'static str {
        GRAPH_LAYOUT_FORCE_DIRECTED
    }

    fn display_name(&self) -> &'static str {
        "Force Directed"
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn execute(
        &self,
        _graph: &mut Graph,
        layout_mode: &LayoutMode,
    ) -> Result<LayoutExecution, String> {
        let resolution = resolve_layout_algorithm(Some(layout_algorithm_id_for_mode(layout_mode)));
        Ok(LayoutExecution {
            resolution,
            changed_positions: 0,
            stable: false,
        })
    }
}

pub(crate) struct ForceDirectedBarnesHutLayout;

impl LayoutAlgorithm for ForceDirectedBarnesHutLayout {
    fn id(&self) -> &'static str {
        GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
    }

    fn display_name(&self) -> &'static str {
        "Force Directed (Barnes-Hut)"
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn execute(
        &self,
        _graph: &mut Graph,
        _layout_mode: &LayoutMode,
    ) -> Result<LayoutExecution, String> {
        let resolution = resolve_layout_algorithm(Some(self.id()));
        Ok(LayoutExecution {
            resolution,
            changed_positions: 0,
            stable: false,
        })
    }
}

pub(crate) struct GridLayout;

impl LayoutAlgorithm for GridLayout {
    fn id(&self) -> &'static str {
        GRAPH_LAYOUT_GRID
    }

    fn display_name(&self) -> &'static str {
        "Grid"
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn execute(
        &self,
        graph: &mut Graph,
        layout_mode: &LayoutMode,
    ) -> Result<LayoutExecution, String> {
        let LayoutMode::Grid { gap } = layout_mode else {
            return Err("grid layout requested with non-grid mode".to_string());
        };
        let changed_positions = apply_grid_layout(graph, *gap);
        Ok(LayoutExecution {
            resolution: resolve_layout_algorithm(Some(self.id())),
            changed_positions,
            stable: true,
        })
    }
}

pub(crate) struct TreeLayout;

impl LayoutAlgorithm for TreeLayout {
    fn id(&self) -> &'static str {
        GRAPH_LAYOUT_TREE
    }

    fn display_name(&self) -> &'static str {
        "Tree"
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn execute(
        &self,
        graph: &mut Graph,
        layout_mode: &LayoutMode,
    ) -> Result<LayoutExecution, String> {
        let LayoutMode::Tree {
            direction,
            layer_gap,
        } = layout_mode
        else {
            return Err("tree layout requested with non-tree mode".to_string());
        };
        let changed_positions = apply_tree_layout(graph, *direction, *layer_gap);
        Ok(LayoutExecution {
            resolution: resolve_layout_algorithm(Some(self.id())),
            changed_positions,
            stable: true,
        })
    }
}

pub(crate) fn layout_algorithm_id_for_mode(layout_mode: &LayoutMode) -> &'static str {
    match layout_mode {
        LayoutMode::Free => GRAPH_LAYOUT_FORCE_DIRECTED,
        LayoutMode::Grid { .. } => GRAPH_LAYOUT_GRID,
        LayoutMode::Tree { .. } => GRAPH_LAYOUT_TREE,
    }
}

pub(crate) fn default_free_layout_algorithm_id() -> String {
    GRAPH_LAYOUT_FORCE_DIRECTED.to_string()
}

pub(crate) fn resolve_layout_algorithm(requested_id: Option<&str>) -> LayoutResolution {
    let requested = requested_id.unwrap_or_default().trim().to_ascii_lowercase();
    let canonical_requested = match requested.as_str() {
        "" => GRAPH_LAYOUT_FORCE_DIRECTED,
        LEGACY_LAYOUT_DEFAULT => GRAPH_LAYOUT_FORCE_DIRECTED,
        LEGACY_LAYOUT_GRID => GRAPH_LAYOUT_GRID,
        other => other,
    };

    let resolved = match canonical_requested {
        GRAPH_LAYOUT_FORCE_DIRECTED => Some(LayoutMode::Free),
        GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT => Some(LayoutMode::Free),
        GRAPH_LAYOUT_GRID => Some(LayoutMode::Grid { gap: 48.0 }),
        GRAPH_LAYOUT_TREE => Some(LayoutMode::Tree {
            direction: Direction::Outgoing,
            layer_gap: 96.0,
        }),
        _ => None,
    };

    let (resolved_id, matched, fallback_used, layout_mode) = if let Some(layout_mode) = resolved {
        (
            canonical_requested.to_string(),
            !requested.is_empty(),
            false,
            layout_mode,
        )
    } else {
        (
            GRAPH_LAYOUT_FORCE_DIRECTED.to_string(),
            false,
            !requested.is_empty(),
            LayoutMode::Free,
        )
    };

    LayoutResolution {
        requested_id: requested,
        resolved_id: resolved_id.clone(),
        matched,
        fallback_used,
        capability: capability_for_algorithm_id(&resolved_id),
        layout_mode,
    }
}

pub(crate) fn capability_for_algorithm_id(algorithm_id: &str) -> LayoutCapability {
    match algorithm_id {
        GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT => LayoutCapability {
            algorithm_id: GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT.to_string(),
            display_name: "Force Directed (Barnes-Hut)".to_string(),
            deterministic: true,
        },
        GRAPH_LAYOUT_GRID => LayoutCapability {
            algorithm_id: GRAPH_LAYOUT_GRID.to_string(),
            display_name: "Grid".to_string(),
            deterministic: true,
        },
        GRAPH_LAYOUT_TREE => LayoutCapability {
            algorithm_id: GRAPH_LAYOUT_TREE.to_string(),
            display_name: "Tree".to_string(),
            deterministic: true,
        },
        _ => LayoutCapability {
            algorithm_id: GRAPH_LAYOUT_FORCE_DIRECTED.to_string(),
            display_name: "Force Directed".to_string(),
            deterministic: true,
        },
    }
}

fn apply_grid_layout(graph: &mut Graph, gap: f32) -> usize {
    let mut keys: Vec<NodeKey> = graph.nodes().map(|(key, _)| key).collect();
    keys.sort_by_key(|key| key.index());
    if keys.is_empty() {
        return 0;
    }

    let columns = (keys.len() as f32).sqrt().ceil().max(1.0) as usize;
    let rows = keys.len().div_ceil(columns);
    let centroid = graph
        .projected_centroid()
        .unwrap_or_else(|| Point2D::new(0.0, 0.0));
    let width = (columns.saturating_sub(1) as f32) * gap;
    let height = (rows.saturating_sub(1) as f32) * gap;
    let origin_x = centroid.x - width * 0.5;
    let origin_y = centroid.y - height * 0.5;

    let mut changed_positions = 0;
    for (index, key) in keys.into_iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let position = Point2D::new(origin_x + col as f32 * gap, origin_y + row as f32 * gap);
        if graph.set_node_projected_position(key, position) {
            changed_positions += 1;
        }
    }
    changed_positions
}

fn apply_tree_layout(graph: &mut Graph, direction: Direction, layer_gap: f32) -> usize {
    let mut all_keys: Vec<NodeKey> = graph.nodes().map(|(key, _)| key).collect();
    all_keys.sort_by_key(|key| key.index());
    if all_keys.is_empty() {
        return 0;
    }

    let roots = root_nodes_for_direction(graph, &all_keys, direction);
    let mut visited = HashMap::<NodeKey, usize>::new();
    let mut queue = VecDeque::new();
    for root in roots {
        if visited.contains_key(&root) {
            continue;
        }
        visited.insert(root, 0);
        queue.push_back(root);
        while let Some(key) = queue.pop_front() {
            let depth = visited[&key];
            let mut neighbors: Vec<NodeKey> = match direction {
                Direction::Outgoing => graph.out_neighbors(key).collect(),
                Direction::Incoming => graph.in_neighbors(key).collect(),
            };
            neighbors.sort_by_key(|key| key.index());
            for neighbor in neighbors {
                if visited.contains_key(&neighbor) {
                    continue;
                }
                visited.insert(neighbor, depth + 1);
                queue.push_back(neighbor);
            }
        }
    }

    for key in all_keys.iter().copied() {
        if visited.contains_key(&key) {
            continue;
        }
        let max_depth = visited.values().copied().max().unwrap_or(0);
        visited.insert(key, max_depth + 1);
    }

    let mut layers = HashMap::<usize, Vec<NodeKey>>::new();
    for (key, depth) in visited {
        layers.entry(depth).or_default().push(key);
    }
    for keys in layers.values_mut() {
        keys.sort_by_key(|key| key.index());
    }

    let max_depth = layers.keys().copied().max().unwrap_or(0);
    let centroid = graph
        .projected_centroid()
        .unwrap_or_else(|| Point2D::new(0.0, 0.0));
    let horizontal_gap = (layer_gap * 1.2).max(48.0);
    let total_height = max_depth as f32 * layer_gap;
    let start_y = centroid.y - total_height * 0.5;
    let direction_sign = if matches!(direction, Direction::Incoming) {
        -1.0
    } else {
        1.0
    };

    let mut changed_positions = 0;
    let mut layer_indices: Vec<usize> = layers.keys().copied().collect();
    layer_indices.sort_unstable();
    for depth in layer_indices {
        let nodes = &layers[&depth];
        let total_width = (nodes.len().saturating_sub(1) as f32) * horizontal_gap;
        let start_x = centroid.x - total_width * 0.5;
        for (index, key) in nodes.iter().enumerate() {
            let position = Point2D::new(
                start_x + index as f32 * horizontal_gap,
                start_y + depth as f32 * layer_gap * direction_sign,
            );
            if graph.set_node_projected_position(*key, position) {
                changed_positions += 1;
            }
        }
    }
    changed_positions
}

fn root_nodes_for_direction(
    graph: &Graph,
    all_keys: &[NodeKey],
    direction: Direction,
) -> Vec<NodeKey> {
    let mut roots: Vec<NodeKey> = all_keys
        .iter()
        .copied()
        .filter(|key| match direction {
            Direction::Outgoing => graph.in_neighbors(*key).next().is_none(),
            Direction::Incoming => graph.out_neighbors(*key).next().is_none(),
        })
        .collect();
    if roots.is_empty() {
        roots = all_keys.to_vec();
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use euclid::default::Point2D;

    fn test_graph() -> Graph {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.test".to_string(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.test".to_string(), Point2D::new(10.0, 0.0));
        let c = graph.add_node("https://c.test".to_string(), Point2D::new(20.0, 0.0));
        let _ = graph.add_edge(a, b, crate::graph::EdgeType::UserGrouped, None);
        let _ = graph.add_edge(b, c, crate::graph::EdgeType::UserGrouped, None);
        graph
    }

    #[test]
    fn layout_resolution_accepts_legacy_aliases() {
        let default_resolution = resolve_layout_algorithm(Some("layout:default"));
        assert_eq!(default_resolution.resolved_id, GRAPH_LAYOUT_FORCE_DIRECTED);

        let barnes_hut_resolution =
            resolve_layout_algorithm(Some(GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT));
        assert_eq!(
            barnes_hut_resolution.resolved_id,
            GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
        );

        let grid_resolution = resolve_layout_algorithm(Some("layout:grid"));
        assert_eq!(grid_resolution.resolved_id, GRAPH_LAYOUT_GRID);
    }

    #[test]
    fn grid_layout_repositions_nodes() {
        let mut graph = test_graph();
        let execution = GridLayout
            .execute(&mut graph, &LayoutMode::Grid { gap: 60.0 })
            .expect("grid layout should execute");

        assert!(execution.changed_positions > 0);
        let positions: Vec<_> = graph
            .nodes()
            .map(|(_, node)| node.projected_position())
            .collect();
        assert!(positions.windows(2).any(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn tree_layout_assigns_layered_y_positions() {
        let mut graph = test_graph();
        let execution = TreeLayout
            .execute(
                &mut graph,
                &LayoutMode::Tree {
                    direction: Direction::Outgoing,
                    layer_gap: 90.0,
                },
            )
            .expect("tree layout should execute");

        assert!(execution.changed_positions > 0);
        let mut ys: Vec<_> = graph
            .nodes()
            .map(|(_, node)| node.projected_position().y)
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);
        assert!(ys.len() >= 2);
    }
}

