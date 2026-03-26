use crate::app::GraphBrowserApp;
use crate::graph::{ClassificationScheme, ClassificationStatus, NodeKey};

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
    /// Number of nodes whose semantic index entry was enriched (at least
    /// partially) from `NodeClassification` records rather than only tags.
    /// When non-zero these nodes participate in semantic placement and gravity
    /// even if they carry no explicit UDC tag.
    pub(crate) classification_enriched_nodes: usize,
}

pub fn reconcile_semantics(
    app: &mut GraphBrowserApp,
    registry: &KnowledgeRegistry,
) -> SemanticReconcileReport {
    if !app.workspace.graph_runtime.semantic_index_dirty {
        return SemanticReconcileReport {
            indexed_nodes: app.workspace.graph_runtime.semantic_index.len(),
            removed_stale_tags: 0,
            changed: false,
            classification_enriched_nodes: 0,
        };
    }

    let previous_index = app.workspace.graph_runtime.semantic_index.clone();
    let removed_stale_tags = 0;

    let mut rebuilt_index = std::collections::HashMap::new();
    let mut classification_enriched_nodes = 0usize;
    for (key, node) in app.workspace.domain.graph.nodes() {
        let mut codes = Vec::new();
        // Index UDC tags directly.
        for tag in &node.tags {
            if let Some(code) = registry.parse(tag) {
                codes.push(code);
            }
        }
        // Also index accepted/verified UDC classifications so that nodes
        // classified via the knowledge-capture pipeline (rather than explicit
        // tag application) participate in semantic placement and clustering.
        let codes_before = codes.len();
        for classification in &node.classifications {
            if matches!(
                classification.status,
                ClassificationStatus::Accepted | ClassificationStatus::Verified
            ) && matches!(classification.scheme, ClassificationScheme::Udc)
                && !node.tags.contains(&classification.value)
            {
                if let Some(code) = registry.parse(&classification.value) {
                    codes.push(code);
                }
            }
        }
        if codes.len() > codes_before {
            classification_enriched_nodes += 1;
        }
        if !codes.is_empty() {
            rebuilt_index.insert(key, SemanticClassVector::from_codes(codes));
        }
    }

    let changed = previous_index != rebuilt_index || removed_stale_tags > 0;
    app.workspace.graph_runtime.semantic_index = rebuilt_index;
    app.workspace.graph_runtime.semantic_index_dirty = false;

    if changed && classification_enriched_nodes > 0 {
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: super::CHANNEL_KNOWLEDGE_CLASSIFICATION_CLUSTERING_APPLIED,
                byte_len: classification_enriched_nodes,
            },
        );
    }

    SemanticReconcileReport {
        indexed_nodes: app.workspace.graph_runtime.semantic_index.len(),
        removed_stale_tags,
        changed,
        classification_enriched_nodes,
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
        .domain
        .graph
        .nodes()
        .filter_map(|(key, node)| node.tags.contains(&canonical_tag).then_some(key))
        .collect::<Vec<_>>();
    matches.sort_by_key(|key| key.index());
    matches
}

pub fn tags_for_node(app: &GraphBrowserApp, key: &NodeKey) -> Vec<String> {
    let Some(node) = app.workspace.domain.graph.get_node(*key) else {
        return Vec::new();
    };
    let mut tags = Vec::new();
    for tag in &node.tag_presentation.ordered_tags {
        if node.tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    let mut remaining = node
        .tags
        .iter()
        .filter(|tag| !tags.contains(tag))
        .cloned()
        .collect::<Vec<_>>();
    remaining.sort();
    tags.extend(remaining);
    tags
}

pub fn suggest_placement_anchor(
    app: &GraphBrowserApp,
    registry: &KnowledgeRegistry,
    key: NodeKey,
) -> Option<NodeKey> {
    let source = app.workspace.graph_runtime.semantic_index.get(&key)?;
    let mut ranked = app
        .workspace
        .graph_runtime
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
    ranked.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.0.index().cmp(&b.0.index()))
    });
    let anchor = ranked.first().map(|(key, _)| *key);
    if anchor.is_some() {
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: super::CHANNEL_KNOWLEDGE_PLACEMENT_ANCHOR_SELECTED,
                byte_len: key.index(),
            },
        );
    }
    anchor
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
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(key, "udc:51".to_string());
        app.workspace.graph_runtime.semantic_index_dirty = true;

        let report = reconcile_semantics(&mut app, &registry);

        assert!(report.changed);
        assert_eq!(report.indexed_nodes, 1);
        assert_eq!(report.classification_enriched_nodes, 0);
        assert!(!app.workspace.graph_runtime.semantic_index_dirty);
        let index = app
            .workspace
            .graph_runtime
            .semantic_index
            .get(&key)
            .unwrap();
        assert_eq!(index.primary_code, Some(CompactCode(vec![5, 1])));
        assert_eq!(index.classes, vec![CompactCode(vec![5, 1])]);

        let stale = NodeKey::new(999_999);
        app.workspace.graph_runtime.semantic_index_dirty = true;
        let report = reconcile_semantics(&mut app, &registry);
        assert_eq!(report.removed_stale_tags, 0);
        assert!(app.workspace.domain.graph.get_node(stale).is_none());
    }

    #[test]
    fn reconcile_indexes_accepted_udc_classification_without_matching_tag() {
        use crate::graph::{
            ClassificationProvenance, ClassificationScheme, ClassificationStatus, NodeClassification,
        };

        let registry = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync(
            "https://example.com/math-classified".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        // Add a classification but no matching tag.
        app.workspace.domain.graph.add_node_classification(
            key,
            NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:51".to_string(),
                label: Some("Mathematics".to_string()),
                confidence: 1.0,
                provenance: ClassificationProvenance::UserAuthored,
                status: ClassificationStatus::Accepted,
                primary: true,
            },
        );
        app.workspace.graph_runtime.semantic_index_dirty = true;

        let report = reconcile_semantics(&mut app, &registry);

        assert!(report.changed);
        assert_eq!(report.indexed_nodes, 1);
        assert_eq!(report.classification_enriched_nodes, 1);
        let index = app
            .workspace
            .graph_runtime
            .semantic_index
            .get(&key)
            .expect("node should be in semantic index");
        assert_eq!(index.classes, vec![CompactCode(vec![5, 1])]);
    }

    #[test]
    fn classification_enriched_node_gets_semantic_placement_anchor() {
        use crate::graph::{
            ClassificationProvenance, ClassificationScheme, ClassificationStatus, NodeClassification,
        };

        let registry = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();

        // Node A: has a UDC tag for mathematics.
        let math_tagged = app.add_node_and_sync(
            "https://example.com/math-tagged".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(math_tagged, "udc:51".to_string());

        // Node B: classified as mathematics via NodeClassification, no tag.
        let math_classified = app.add_node_and_sync(
            "https://example.com/math-classified".to_string(),
            euclid::default::Point2D::new(50.0, 0.0),
        );
        app.workspace.domain.graph.add_node_classification(
            math_classified,
            NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:519.6".to_string(),
                label: Some("Computational mathematics".to_string()),
                confidence: 1.0,
                provenance: ClassificationProvenance::InheritedFromSource,
                status: ClassificationStatus::Accepted,
                primary: true,
            },
        );

        // Node C: tagged as music — semantically distant.
        let music = app.add_node_and_sync(
            "https://example.com/music".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(music, "udc:78".to_string());

        app.workspace.graph_runtime.semantic_index_dirty = true;
        let _ = reconcile_semantics(&mut app, &registry);

        // B (classified math) should anchor to A (tagged math), not C (music).
        assert_eq!(
            suggest_placement_anchor(&app, &registry, math_classified),
            Some(math_tagged)
        );
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

        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(a, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(a, "udc:519.6".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(b, "udc:78".to_string());

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

        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(math, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(numerical, "udc:519.6".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(music, "udc:78".to_string());
        app.workspace.graph_runtime.semantic_index_dirty = true;
        let _ = reconcile_semantics(&mut app, &registry);

        assert_eq!(
            suggest_placement_anchor(&app, &registry, numerical),
            Some(math)
        );
    }
}
