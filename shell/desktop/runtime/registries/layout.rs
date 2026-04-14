use std::collections::HashMap;

use crate::app::graph_layout::{
    ForceDirectedBarnesHutLayout, ForceDirectedLayout, GRAPH_LAYOUT_FORCE_DIRECTED,
    GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT, GRAPH_LAYOUT_GRID, GRAPH_LAYOUT_TREE, GridLayout,
    LayoutAlgorithm, LayoutCapability, LayoutExecution, LayoutResolution, TreeLayout,
    capability_for_algorithm_id, resolve_layout_algorithm,
};
use crate::graph::Graph;

pub(crate) struct LayoutRegistry {
    active_algorithm_id: String,
    algorithms: HashMap<String, Box<dyn LayoutAlgorithm>>,
}

impl Default for LayoutRegistry {
    fn default() -> Self {
        let mut algorithms: HashMap<String, Box<dyn LayoutAlgorithm>> = HashMap::new();
        algorithms.insert(
            GRAPH_LAYOUT_FORCE_DIRECTED.to_string(),
            Box::new(ForceDirectedLayout),
        );
        algorithms.insert(
            GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT.to_string(),
            Box::new(ForceDirectedBarnesHutLayout),
        );
        algorithms.insert(GRAPH_LAYOUT_GRID.to_string(), Box::new(GridLayout));
        algorithms.insert(GRAPH_LAYOUT_TREE.to_string(), Box::new(TreeLayout));
        Self {
            active_algorithm_id: GRAPH_LAYOUT_FORCE_DIRECTED.to_string(),
            algorithms,
        }
    }
}

impl LayoutRegistry {
    pub(crate) fn active_algorithm_id(&self) -> &str {
        &self.active_algorithm_id
    }

    pub(crate) fn describe_algorithm(&self, algorithm_id: Option<&str>) -> LayoutCapability {
        let resolution = self.resolve_algorithm(algorithm_id);
        resolution.capability
    }

    pub(crate) fn set_active_algorithm(&mut self, algorithm_id: Option<&str>) -> LayoutResolution {
        let resolution = self.resolve_algorithm(algorithm_id);
        self.active_algorithm_id = resolution.resolved_id.clone();
        resolution
    }

    pub(crate) fn active_algorithm(&self) -> LayoutResolution {
        self.resolve_algorithm(Some(&self.active_algorithm_id))
    }

    pub(crate) fn resolve_algorithm(&self, algorithm_id: Option<&str>) -> LayoutResolution {
        let mut resolution = resolve_layout_algorithm(algorithm_id);
        if !self.algorithms.contains_key(&resolution.resolved_id) {
            resolution = resolve_layout_algorithm(Some(GRAPH_LAYOUT_FORCE_DIRECTED));
            resolution.fallback_used = true;
        }
        resolution.capability = capability_for_algorithm_id(&resolution.resolved_id);
        resolution
    }

    pub(crate) fn apply_algorithm_to_graph(
        &mut self,
        graph: &mut Graph,
        algorithm_id: Option<&str>,
    ) -> Result<LayoutExecution, String> {
        let resolution = self.set_active_algorithm(algorithm_id);
        let algorithm = self
            .algorithms
            .get(&resolution.resolved_id)
            .ok_or_else(|| {
                format!(
                    "layout algorithm '{}' is not registered",
                    resolution.resolved_id
                )
            })?;
        let mut execution = algorithm.execute(graph, &resolution.layout_mode)?;
        execution.resolution = resolution;
        Ok(execution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use euclid::default::Point2D;

    fn test_graph() -> Graph {
        let mut graph = Graph::new();
        graph.add_node("https://a.test".into(), Point2D::new(0.0, 0.0));
        graph.add_node("https://b.test".into(), Point2D::new(10.0, 0.0));
        graph
    }

    #[test]
    fn layout_registry_defaults_to_force_directed() {
        let registry = LayoutRegistry::default();
        let resolution = registry.active_algorithm();

        assert_eq!(registry.active_algorithm_id(), GRAPH_LAYOUT_FORCE_DIRECTED);
        assert_eq!(resolution.resolved_id, GRAPH_LAYOUT_FORCE_DIRECTED);
    }

    #[test]
    fn layout_registry_falls_back_for_unknown_algorithm() {
        let mut registry = LayoutRegistry::default();
        let resolution = registry.set_active_algorithm(Some("graph_layout:missing"));

        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, GRAPH_LAYOUT_FORCE_DIRECTED);
    }

    #[test]
    fn layout_registry_applies_grid_positions() {
        let mut registry = LayoutRegistry::default();
        let mut graph = test_graph();

        let execution = registry
            .apply_algorithm_to_graph(&mut graph, Some(GRAPH_LAYOUT_GRID))
            .expect("grid layout should execute");

        assert_eq!(execution.resolution.resolved_id, GRAPH_LAYOUT_GRID);
        assert!(execution.changed_positions > 0);
    }

    #[test]
    fn layout_registry_accepts_barnes_hut_force_directed() {
        let mut registry = LayoutRegistry::default();
        let resolution =
            registry.set_active_algorithm(Some(GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT));

        assert_eq!(
            resolution.resolved_id,
            GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
        );
        assert_eq!(
            registry.active_algorithm_id(),
            GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT
        );
    }
}

