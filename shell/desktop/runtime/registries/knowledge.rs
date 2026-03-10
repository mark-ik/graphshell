use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;

#[cfg(test)]
pub(crate) use crate::registries::atomic::knowledge::CompactCode;
pub(crate) use crate::registries::atomic::knowledge::{
    KnowledgeRegistry, SemanticClassVector, TagValidationResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticReconcileReport {
    pub(crate) indexed_nodes: usize,
    pub(crate) removed_stale_tags: usize,
    pub(crate) changed: bool,
}

pub fn reconcile_semantics(
    app: &mut GraphBrowserApp,
    registry: &KnowledgeRegistry,
) -> SemanticReconcileReport {
    if !app.workspace.semantic_index_dirty {
        return SemanticReconcileReport {
            indexed_nodes: app.workspace.semantic_index.len(),
            removed_stale_tags: 0,
            changed: false,
        };
    }

    let previous_index = app.workspace.semantic_index.clone();
    let previous_tag_len = app.workspace.semantic_tags.len();
    let valid_keys: std::collections::HashSet<_> = app
        .workspace
        .domain
        .graph
        .nodes()
        .map(|(key, _)| key)
        .collect();

    app.workspace
        .semantic_tags
        .retain(|key, _| valid_keys.contains(key));
    let removed_stale_tags = previous_tag_len.saturating_sub(app.workspace.semantic_tags.len());

    let mut rebuilt_index = std::collections::HashMap::new();
    for (&key, tags) in &app.workspace.semantic_tags {
        let mut codes = Vec::new();
        for tag in tags {
            if let Some(code) = registry.parse(tag) {
                codes.push(code);
            }
        }
        if !codes.is_empty() {
            rebuilt_index.insert(key, SemanticClassVector::from_codes(codes));
        }
    }

    let changed = previous_index != rebuilt_index || removed_stale_tags > 0;
    app.workspace.semantic_index = rebuilt_index;
    app.workspace.semantic_index_dirty = false;

    SemanticReconcileReport {
        indexed_nodes: app.workspace.semantic_index.len(),
        removed_stale_tags,
        changed,
    }
}

pub fn query_by_tag(
    app: &GraphBrowserApp,
    registry: &KnowledgeRegistry,
    tag: &str,
) -> Vec<NodeKey> {
    let Some(canonical_tag) = registry.canonicalize_tag(tag) else {
        return Vec::new();
    };

    let mut matches = app
        .workspace
        .semantic_tags
        .iter()
        .filter_map(|(key, tags)| tags.contains(&canonical_tag).then_some(*key))
        .collect::<Vec<_>>();
    matches.sort_by_key(|key| key.index());
    matches
}

pub fn tags_for_node(app: &GraphBrowserApp, key: &NodeKey) -> Vec<String> {
    let mut tags = app
        .workspace
        .semantic_tags
        .get(key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    tags.sort();
    tags
}

pub fn suggest_placement_anchor(
    app: &GraphBrowserApp,
    registry: &KnowledgeRegistry,
    key: NodeKey,
) -> Option<NodeKey> {
    let source = app.workspace.semantic_index.get(&key)?;
    let mut ranked = app
        .workspace
        .semantic_index
        .iter()
        .filter_map(|(candidate_key, candidate)| {
            if *candidate_key == key {
                return None;
            }
            let distance = semantic_vector_distance(registry, source, candidate)?;
            Some((*candidate_key, distance))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.index().cmp(&b.0.index())));
    ranked.first().map(|(key, _)| *key)
}

fn semantic_vector_distance(
    registry: &KnowledgeRegistry,
    a: &SemanticClassVector,
    b: &SemanticClassVector,
) -> Option<f32> {
    let mut best: Option<f32> = None;
    for left in &a.classes {
        for right in &b.classes {
            let distance = registry.distance(left, right);
            best = Some(match best {
                Some(current) => current.min(distance),
                None => distance,
            });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeKey;

    #[test]
    fn reconcile_updates_semantic_index_and_clears_dirty_flag() {
        let registry = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync(
            "https://example.com".to_string(),
            euclid::default::Point2D::new(10.0, 10.0),
        );
        app.workspace
            .semantic_tags
            .insert(key, ["udc:51".to_string()].into_iter().collect());
        app.workspace.semantic_index_dirty = true;

        let report = reconcile_semantics(&mut app, &registry);

        assert!(report.changed);
        assert_eq!(report.indexed_nodes, 1);
        assert!(!app.workspace.semantic_index_dirty);
        let index = app.workspace.semantic_index.get(&key).unwrap();
        assert_eq!(index.primary_code, Some(CompactCode(vec![5, 1])));
        assert_eq!(index.classes, vec![CompactCode(vec![5, 1])]);

        let stale = NodeKey::new(999_999);
        app.workspace
            .semantic_tags
            .insert(stale, ["udc:7".to_string()].into_iter().collect());
        app.workspace.semantic_index_dirty = true;
        let report = reconcile_semantics(&mut app, &registry);
        assert_eq!(report.removed_stale_tags, 1);
        assert!(!app.workspace.semantic_tags.contains_key(&stale));
    }

    #[test]
    fn query_by_tag_and_tags_for_node_use_canonicalized_knowledge_tags() {
        let registry = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://example.com/math".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://example.com/music".to_string(),
            euclid::default::Point2D::new(10.0, 10.0),
        );

        app.workspace
            .semantic_tags
            .insert(a, ["udc:51".to_string(), "udc:519.6".to_string()].into_iter().collect());
        app.workspace
            .semantic_tags
            .insert(b, ["udc:78".to_string()].into_iter().collect());

        let matches = query_by_tag(&app, &registry, "51");
        assert_eq!(matches, vec![a]);

        let tags = tags_for_node(&app, &a);
        assert_eq!(tags, vec!["udc:51".to_string(), "udc:519.6".to_string()]);
    }

    #[test]
    fn suggest_placement_anchor_prefers_semantic_kin() {
        let registry = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let math = app.add_node_and_sync(
            "https://example.com/math".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let numerical = app.add_node_and_sync(
            "https://example.com/numerical".to_string(),
            euclid::default::Point2D::new(20.0, 0.0),
        );
        let music = app.add_node_and_sync(
            "https://example.com/music".to_string(),
            euclid::default::Point2D::new(40.0, 0.0),
        );

        app.workspace
            .semantic_tags
            .insert(math, ["udc:51".to_string()].into_iter().collect());
        app.workspace
            .semantic_tags
            .insert(numerical, ["udc:519.6".to_string()].into_iter().collect());
        app.workspace
            .semantic_tags
            .insert(music, ["udc:78".to_string()].into_iter().collect());
        app.workspace.semantic_index_dirty = true;
        let _ = reconcile_semantics(&mut app, &registry);

        assert_eq!(
            suggest_placement_anchor(&app, &registry, numerical),
            Some(math)
        );
    }
}
