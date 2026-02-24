use crate::app::GraphBrowserApp;

pub(crate) use crate::registries::atomic::knowledge::{
    CompactCode,
    KnowledgeRegistry,
};

/// Reconciliation function: updates the app's semantic index based on node tags.
/// This respects the "Data vs System" split: App owns Data, Registry owns Logic.
pub fn reconcile_semantics(app: &mut GraphBrowserApp, registry: &KnowledgeRegistry) {
    if !app.semantic_index_dirty {
        return;
    }

    app.semantic_tags
        .retain(|key, _| app.graph.get_node(*key).is_some());

    app.semantic_index.clear();
    for (&key, tags) in &app.semantic_tags {
        for tag in tags {
            if let Some(code) = registry.parse(tag) {
                app.semantic_index.insert(key, code);
                break;
            }
        }
    }
    app.semantic_index_dirty = false;
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
        app.semantic_tags
            .insert(key, ["udc:51".to_string()].into_iter().collect());
        app.semantic_index_dirty = true;

        reconcile_semantics(&mut app, &registry);

        assert!(!app.semantic_index_dirty);
        assert_eq!(app.semantic_index.get(&key), Some(&CompactCode(vec![5, 1])));

        let stale = NodeKey::new(999_999);
        app.semantic_tags
            .insert(stale, ["udc:7".to_string()].into_iter().collect());
        app.semantic_index_dirty = true;
        reconcile_semantics(&mut app, &registry);
        assert!(!app.semantic_tags.contains_key(&stale));
    }
}
