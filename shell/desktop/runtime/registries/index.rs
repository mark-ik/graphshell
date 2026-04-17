use std::collections::{HashMap, HashSet};

use crate::app::{
    GraphBrowserApp, user_visible_node_title_from_data, user_visible_node_url_from_data,
};
use crate::graph::NodeKey;
use crate::services::search::{fuzzy_match_items, fuzzy_match_node_keys};

use super::knowledge::KnowledgeRegistry;

pub(crate) const INDEX_PROVIDER_LOCAL: &str = "index:local";
pub(crate) const INDEX_PROVIDER_HISTORY: &str = "index:history";
pub(crate) const INDEX_PROVIDER_KNOWLEDGE: &str = "index:knowledge";

pub(crate) trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn search(
        &self,
        app: &GraphBrowserApp,
        knowledge: &KnowledgeRegistry,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SearchResultKind {
    Node(NodeKey),
    HistoryUrl(String),
    KnowledgeTag { code: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SearchResult {
    pub(crate) title: String,
    pub(crate) url: Option<String>,
    pub(crate) snippet: Option<String>,
    pub(crate) source: String,
    pub(crate) relevance: f32,
    pub(crate) semantic_tags: Vec<String>,
    pub(crate) kind: SearchResultKind,
}

pub(crate) struct IndexRegistry {
    providers: HashMap<String, Box<dyn SearchProvider>>,
}

impl IndexRegistry {
    pub(crate) fn register_provider(
        &mut self,
        provider: Box<dyn SearchProvider>,
    ) -> Result<(), String> {
        let provider_id = provider.id().to_ascii_lowercase();
        if self.providers.contains_key(&provider_id) {
            return Err(format!("search provider already registered: {provider_id}"));
        }
        self.providers.insert(provider_id, provider);
        Ok(())
    }

    pub(crate) fn unregister_provider(&mut self, provider_id: &str) -> bool {
        self.providers
            .remove(&provider_id.trim().to_ascii_lowercase())
            .is_some()
    }

    pub(crate) fn search(
        &self,
        app: &GraphBrowserApp,
        knowledge: &KnowledgeRegistry,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let mut merged = Vec::new();
        if let Some(provider) = self.providers.get(INDEX_PROVIDER_LOCAL) {
            merged.extend(provider.search(app, knowledge, trimmed, limit));
        }

        for provider_id in [INDEX_PROVIDER_HISTORY, INDEX_PROVIDER_KNOWLEDGE] {
            if let Some(provider) = self.providers.get(provider_id) {
                merged.extend(provider.search(app, knowledge, trimmed, limit));
            }
        }

        merged.sort_by(|a, b| {
            b.relevance
                .total_cmp(&a.relevance)
                .then_with(|| provider_sort_key(&a.source).cmp(&provider_sort_key(&b.source)))
                .then_with(|| a.title.cmp(&b.title))
        });
        merged.truncate(limit);
        merged
    }

    #[cfg(test)]
    fn clear_optional_providers(&mut self) {
        self.providers
            .retain(|provider_id, _| provider_id == INDEX_PROVIDER_LOCAL);
    }
}

impl Default for IndexRegistry {
    fn default() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        registry
            .register_provider(Box::new(LocalSearchProvider))
            .expect("local provider registration should succeed");
        registry
            .register_provider(Box::new(HistorySearchProvider))
            .expect("history provider registration should succeed");
        registry
            .register_provider(Box::new(KnowledgeSearchProvider))
            .expect("knowledge provider registration should succeed");
        registry
    }
}

struct LocalSearchProvider;

#[derive(Clone)]
struct LocalSearchCandidate {
    key: NodeKey,
    text: String,
}

impl AsRef<str> for LocalSearchCandidate {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

fn import_record_search_text(app: &GraphBrowserApp, key: NodeKey) -> String {
    app.domain_graph()
        .import_record_summaries_for_node(key)
        .into_iter()
        .map(|record| {
            format!(
                "{} {} {} {}",
                record.source_label,
                record.source_id,
                record.record_id,
                crate::graph::format_imported_at_secs(record.imported_at_secs),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn import_semantic_tags_for_node(app: &GraphBrowserApp, key: NodeKey) -> Vec<String> {
    let mut tags = app
        .domain_graph()
        .import_record_summaries_for_node(key)
        .into_iter()
        .flat_map(|record| {
            [
                "family:imported".to_string(),
                format!("import:{}", record.source_id),
                format!("import-record:{}", record.record_id),
            ]
        })
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    tags
}

fn local_result_snippet(app: &GraphBrowserApp, key: NodeKey, url: &str) -> String {
    let import_records = app.domain_graph().import_record_summaries_for_node(key);
    if import_records.is_empty() {
        return url.to_string();
    }

    let import_summary = import_records
        .into_iter()
        .map(|record| {
            format!(
                "{} ({}, {})",
                record.source_label,
                crate::graph::format_imported_at_secs(record.imported_at_secs),
                record.record_id,
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("{url} Imported from {import_summary}")
}

impl SearchProvider for LocalSearchProvider {
    fn id(&self) -> &'static str {
        INDEX_PROVIDER_LOCAL
    }

    fn display_name(&self) -> &'static str {
        "Local Graph"
    }

    fn search(
        &self,
        app: &GraphBrowserApp,
        _knowledge: &KnowledgeRegistry,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let trimmed = query.trim();
        if let Some(udc_query) = parse_udc_class_query(trimmed) {
            return local_udc_class_matches(app, &udc_query, limit);
        }

        let candidates: Vec<LocalSearchCandidate> = app
            .domain_graph()
            .nodes()
            .map(|(key, node)| {
                let visible_title = user_visible_node_title_from_data(node);
                let visible_url = user_visible_node_url_from_data(node);
                let semantic_tags = app.canonical_tags_for_node_sorted(key).join(" ");
                let import_search = import_record_search_text(app, key);
                LocalSearchCandidate {
                    key,
                    text: format!(
                        "{} {} {} {}",
                        visible_title, visible_url, semantic_tags, import_search
                    ),
                }
            })
            .collect();

        let matched_keys = if trimmed.starts_with('#')
            || trimmed
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == ':' || c == '/')
        {
            fuzzy_match_items(candidates, trimmed)
                .into_iter()
                .map(|candidate| candidate.key)
                .collect::<Vec<_>>()
        } else {
            let matched_from_candidates = fuzzy_match_items(candidates.clone(), trimmed)
                .into_iter()
                .map(|candidate| candidate.key)
                .collect::<Vec<_>>();
            if matched_from_candidates.is_empty() {
                fuzzy_match_node_keys(app.domain_graph(), trimmed)
            } else {
                matched_from_candidates
            }
        };
        matched_keys
            .into_iter()
            .take(limit)
            .enumerate()
            .filter_map(|(idx, key)| {
                let node = app.domain_graph().get_node(key)?;
                let visible_title = user_visible_node_title_from_data(node);
                let visible_url = user_visible_node_url_from_data(node);
                let mut semantic_tags = app.canonical_tags_for_node_sorted(key);
                semantic_tags.extend(import_semantic_tags_for_node(app, key));
                semantic_tags.sort();
                semantic_tags.dedup();
                Some(SearchResult {
                    title: visible_title,
                    url: Some(visible_url.clone()),
                    snippet: Some(local_result_snippet(app, key, &visible_url)),
                    source: self.id().to_string(),
                    relevance: 1.0 - (idx as f32 * 0.01),
                    semantic_tags,
                    kind: SearchResultKind::Node(key),
                })
            })
            .collect()
    }
}

fn parse_udc_class_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    let udc_query = trimmed
        .strip_prefix("facet:udc_classes=")
        .unwrap_or(trimmed)
        .trim()
        .to_ascii_lowercase();

    if udc_query.starts_with("udc:") && udc_query.len() > "udc:".len() {
        Some(udc_query)
    } else {
        None
    }
}

fn local_udc_class_matches(
    app: &GraphBrowserApp,
    udc_query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let mut matches = app
        .domain_graph()
        .nodes()
        .filter_map(|(key, node)| {
            let semantic_tags = app.canonical_tags_for_node_sorted(key);
            let visible_title = user_visible_node_title_from_data(node);
            let visible_url = user_visible_node_url_from_data(node);
            let match_rank = semantic_tags
                .iter()
                .filter_map(|tag| hierarchical_udc_match_rank(tag, udc_query))
                .min()?;
            Some((key, visible_title, visible_url, semantic_tags, match_rank))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| {
        a.4.cmp(&b.4)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    matches
        .into_iter()
        .take(limit)
        .map(
            |(key, title, url, semantic_tags, match_rank)| SearchResult {
                title: if title.trim().is_empty() {
                    url.clone()
                } else {
                    title
                },
                url: Some(url.clone()),
                snippet: Some(local_result_snippet(app, key, &url)),
                source: INDEX_PROVIDER_LOCAL.to_string(),
                relevance: 1.0 - ((match_rank.min(50) as f32) * 0.01),
                semantic_tags: {
                    let mut semantic_tags = semantic_tags;
                    semantic_tags.extend(import_semantic_tags_for_node(app, key));
                    semantic_tags.sort();
                    semantic_tags.dedup();
                    semantic_tags
                },
                kind: SearchResultKind::Node(key),
            },
        )
        .collect()
}

fn hierarchical_udc_match_rank(tag: &str, udc_query: &str) -> Option<usize> {
    let normalized_tag = tag.trim().to_ascii_lowercase();
    if normalized_tag == udc_query {
        return Some(0);
    }
    if normalized_tag.starts_with(udc_query) {
        return Some(normalized_tag.len().saturating_sub(udc_query.len()) + 1);
    }
    None
}

struct HistorySearchProvider;

impl SearchProvider for HistorySearchProvider {
    fn id(&self) -> &'static str {
        INDEX_PROVIDER_HISTORY
    }

    fn display_name(&self) -> &'static str {
        "History"
    }

    fn search(
        &self,
        app: &GraphBrowserApp,
        _knowledge: &KnowledgeRegistry,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let normalized_query = query.trim().to_ascii_lowercase();
        let mut seen_urls = HashSet::new();
        let mut results = Vec::new();

        for (_key, node) in app.domain_graph().nodes() {
            for url in node.history_entries() {
                let combined = format!("{} {}", node.title, url).to_ascii_lowercase();
                let Some(relevance) = text_relevance(&normalized_query, &combined) else {
                    continue;
                };
                if !seen_urls.insert(url.clone()) {
                    continue;
                }
                results.push(SearchResult {
                    title: if node.title.trim().is_empty() {
                        url.clone()
                    } else {
                        format!("{} history", node.title)
                    },
                    url: Some(url.clone()),
                    snippet: Some(format!("Visited from node '{}'", node.title)),
                    source: self.id().to_string(),
                    relevance,
                    semantic_tags: Vec::new(),
                    kind: SearchResultKind::HistoryUrl(url.clone()),
                });
            }
        }

        results.sort_by(|a, b| {
            b.relevance
                .total_cmp(&a.relevance)
                .then_with(|| a.title.cmp(&b.title))
        });
        results.truncate(limit);
        results
    }
}

struct KnowledgeSearchProvider;

impl SearchProvider for KnowledgeSearchProvider {
    fn id(&self) -> &'static str {
        INDEX_PROVIDER_KNOWLEDGE
    }

    fn display_name(&self) -> &'static str {
        "Knowledge"
    }

    fn search(
        &self,
        _app: &GraphBrowserApp,
        knowledge: &KnowledgeRegistry,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        knowledge
            .search(query)
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(idx, entry)| SearchResult {
                title: entry.label.clone(),
                url: None,
                snippet: Some(format!("UDC {}", entry.code)),
                source: self.id().to_string(),
                relevance: 0.9 - (idx as f32 * 0.01),
                semantic_tags: vec![format!("udc:{}", entry.code)],
                kind: SearchResultKind::KnowledgeTag { code: entry.code },
            })
            .collect()
    }
}

fn provider_sort_key(provider_id: &str) -> usize {
    match provider_id {
        INDEX_PROVIDER_LOCAL => 0,
        INDEX_PROVIDER_HISTORY => 1,
        INDEX_PROVIDER_KNOWLEDGE => 2,
        _ => usize::MAX,
    }
}

fn text_relevance(query: &str, haystack: &str) -> Option<f32> {
    if query.is_empty() {
        return None;
    }
    if haystack == query {
        return Some(0.95);
    }
    if haystack.starts_with(query) {
        return Some(0.9);
    }
    if haystack.contains(query) {
        let density = query.len() as f32 / haystack.len().max(query.len()) as f32;
        return Some((0.6 + density).min(0.89));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ImportRecord, ImportRecordMembership};
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use euclid::default::Point2D;
    use servo::WebViewId;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }
    #[test]
    fn index_registry_fans_out_to_local_history_and_knowledge_providers() {
        let registry = IndexRegistry::default();
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://history.example/math".into(),
            Point2D::new(0.0, 0.0),
        );
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Mathematics Notes".into();
            node.replace_history_state(vec!["https://history.example/math".to_string()], 0);
        }
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(key, "udc:51".to_string());

        let results = registry.search(&app, &knowledge, "math", 10);
        let sources = results
            .into_iter()
            .map(|result| result.source)
            .collect::<HashSet<_>>();

        assert!(sources.contains(INDEX_PROVIDER_LOCAL));
        assert!(sources.contains(INDEX_PROVIDER_HISTORY));
        assert!(sources.contains(INDEX_PROVIDER_KNOWLEDGE));
    }

    #[test]
    fn index_registry_keeps_local_floor_when_optional_providers_are_removed() {
        let mut registry = IndexRegistry::default();
        registry.clear_optional_providers();
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example Handle".into();
        }

        let results = registry.search(&app, &knowledge, "example handle", 10);
        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .all(|result| result.source == INDEX_PROVIDER_LOCAL)
        );
        assert!(
            results
                .iter()
                .any(|result| matches!(result.kind, SearchResultKind::Node(found) if found == key))
        );
    }

    #[test]
    fn local_search_provider_matches_semantic_tags_for_knowledge_queries() {
        let provider = LocalSearchProvider;
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com/math".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Numerical Methods".into();
        }
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(key, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(key, "udc:519.6".to_string());

        let results = provider.search(&app, &knowledge, "udc:519.6", 10);

        assert!(
            results
                .iter()
                .any(|result| matches!(result.kind, SearchResultKind::Node(found) if found == key))
        );
    }

    #[test]
    fn local_search_provider_matches_udc_descendants_for_parent_class_queries() {
        let provider = LocalSearchProvider;
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let descendant = app.workspace.domain.graph.add_node(
            "https://example.com/descendant".into(),
            Point2D::new(0.0, 0.0),
        );
        let ancestor_only = app.workspace.domain.graph.add_node(
            "https://example.com/ancestor".into(),
            Point2D::new(10.0, 0.0),
        );
        let unrelated = app.workspace.domain.graph.add_node(
            "https://example.com/unrelated".into(),
            Point2D::new(20.0, 0.0),
        );
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(descendant, "udc:519.6".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(ancestor_only, "udc:5".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(unrelated, "udc:62".to_string());

        let results = provider.search(&app, &knowledge, "udc:51", 10);

        assert!(results.iter().any(
            |result| matches!(result.kind, SearchResultKind::Node(found) if found == descendant)
        ));
        assert!(!results.iter().any(
            |result| matches!(result.kind, SearchResultKind::Node(found) if found == ancestor_only)
        ));
        assert!(!results.iter().any(
            |result| matches!(result.kind, SearchResultKind::Node(found) if found == unrelated)
        ));
    }

    #[test]
    fn local_search_provider_matches_explicit_udc_class_facet_queries() {
        let provider = LocalSearchProvider;
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com/facet".into(), Point2D::new(0.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(key, "udc:519.6".to_string());

        let results = provider.search(&app, &knowledge, "facet:udc_classes=udc:51", 10);

        assert!(
            results
                .iter()
                .any(|result| matches!(result.kind, SearchResultKind::Node(found) if found == key))
        );
    }

    #[test]
    fn local_search_provider_matches_import_record_metadata() {
        let provider = LocalSearchProvider;
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://example.com/imported".into(),
            Point2D::new(0.0, 0.0),
        );
        let node_id = app
            .workspace
            .domain
            .graph
            .get_node(key)
            .expect("node")
            .id
            .to_string();
        assert!(
            app.workspace
                .domain
                .graph
                .set_import_records(vec![ImportRecord {
                    record_id: "import-record:firefox-bookmarks-2026-03-17".to_string(),
                    source_id: "import:firefox-bookmarks".to_string(),
                    source_label: "Firefox bookmarks".to_string(),
                    imported_at_secs: 1_763_500_800,
                    memberships: vec![ImportRecordMembership {
                        node_id,
                        suppressed: false,
                    }],
                }])
        );

        let results = provider.search(&app, &knowledge, "firefox bookmarks", 10);
        let imported = results
            .into_iter()
            .find(|result| matches!(result.kind, SearchResultKind::Node(found) if found == key))
            .expect("imported node result");

        assert!(
            imported
                .semantic_tags
                .iter()
                .any(|tag| tag == "family:imported")
        );
        assert!(
            imported
                .snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("Firefox bookmarks"))
        );
        assert!(
            imported.snippet.as_deref().is_some_and(
                |snippet| snippet.contains("import-record:firefox-bookmarks-2026-03-17")
            )
        );
    }

    #[test]
    fn local_search_provider_uses_clip_facet_display_metadata() {
        let provider = LocalSearchProvider;
        let knowledge = KnowledgeRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let source_key =
            app.add_node_and_sync("https://example.com/source".into(), Point2D::new(0.0, 0.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, source_key);
        let clip_key = app
            .create_clip_node_from_capture(&crate::app::ClipCaptureData {
                webview_id,
                source_url: "https://example.com/source".to_string(),
                page_title: Some("Example Source".to_string()),
                clip_title: "Indexed Clip".to_string(),
                outer_html: "<article><h2>Indexed Clip</h2></article>".to_string(),
                text_excerpt: "Indexed Clip excerpt".to_string(),
                tag_name: "article".to_string(),
                href: None,
                image_url: None,
                dom_path: Some("body > article:nth-of-type(1)".to_string()),
            })
            .expect("clip node should be created");

        let results = provider.search(&app, &knowledge, "example.com/source", 10);
        let clip_result = results
            .into_iter()
            .find(|result| result.kind == SearchResultKind::Node(clip_key))
            .expect("clip result should be indexed");

        assert_eq!(clip_result.title, "Indexed Clip");
        assert_eq!(
            clip_result.url.as_deref(),
            Some("https://example.com/source")
        );
        assert!(
            clip_result
                .snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("https://example.com/source"))
        );
    }
}
