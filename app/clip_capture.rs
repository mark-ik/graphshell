use base64::Engine;
use euclid::default::Point2D;

use crate::app::{GraphBrowserApp, RendererId};
use crate::graph::NodeKey;
use crate::model::graph::{ClassificationProvenance, ClassificationStatus, NodeClassification};

const CLIP_EDGE_LABEL: &str = "clip-source";
const CLIP_TITLE_FALLBACK: &str = "Clipped element";
const CLIP_TEXT_LIMIT: usize = 80;
const CLIP_GRID_COLUMNS: usize = 3;
const CLIP_GRID_X_SPACING: f32 = 210.0;
const CLIP_GRID_Y_SPACING: f32 = 138.0;
const CLIP_GRID_X_OFFSET: f32 = 180.0;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClipCaptureData {
    pub webview_id: RendererId,
    pub source_url: String,
    pub page_title: Option<String>,
    pub clip_title: String,
    pub outer_html: String,
    pub text_excerpt: String,
    pub tag_name: String,
    pub href: Option<String>,
    pub image_url: Option<String>,
    pub dom_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipInspectorFilter {
    All,
    Text,
    Link,
    Image,
    Structure,
    Media,
}

#[derive(Debug, Clone)]
pub struct ClipInspectorState {
    pub webview_id: RendererId,
    pub source_url: String,
    pub page_title: Option<String>,
    pub search_query: String,
    pub filter: ClipInspectorFilter,
    pub selected_index: usize,
    pub captures: Vec<ClipCaptureData>,
    pub pointer_stack: Vec<ClipCaptureData>,
    pub pointer_stack_index: usize,
    pub highlight_dirty: bool,
}

impl GraphBrowserApp {
    pub fn create_clip_node_from_capture(
        &mut self,
        capture: &ClipCaptureData,
    ) -> Result<NodeKey, String> {
        let (source_key, source_position) = self.clip_source_context(capture.webview_id)?;
        let clip_key = self.create_clip_node_at_position(
            source_key,
            capture,
            Point2D::new(source_position.x + 140.0, source_position.y + 48.0),
        );
        self.select_node(clip_key, false);
        Ok(clip_key)
    }

    pub fn create_clip_nodes_from_captures(
        &mut self,
        captures: &[ClipCaptureData],
    ) -> Result<Vec<NodeKey>, String> {
        if captures.is_empty() {
            return Err("no clip captures were returned".to_string());
        }

        let (source_key, source_position) = self.clip_source_context(captures[0].webview_id)?;
        if captures
            .iter()
            .any(|capture| capture.webview_id != captures[0].webview_id)
        {
            return Err("clip batch mixed captures from different source webviews".to_string());
        }

        let mut clip_keys = Vec::with_capacity(captures.len());
        for (index, capture) in captures.iter().enumerate() {
            let clip_key = self.create_clip_node_at_position(
                source_key,
                capture,
                exploded_clip_position(source_position, index, captures.len()),
            );
            clip_keys.push(clip_key);
        }

        if let Some((first, rest)) = clip_keys.split_first() {
            self.select_node(*first, false);
            for key in rest {
                self.select_node(*key, true);
            }
        }

        Ok(clip_keys)
    }

    pub fn open_clip_inspector(&mut self, captures: Vec<ClipCaptureData>) -> Result<(), String> {
        let Some(first) = captures.first() else {
            return Err("no clip captures were returned".to_string());
        };
        self.workspace
            .graph_runtime
            .pending_clip_inspector_highlight_clear = None;
        self.workspace.chrome_ui.show_clip_inspector = true;
        self.workspace.chrome_ui.show_command_palette = false;
        self.workspace.chrome_ui.show_context_palette = false;
        self.workspace.chrome_ui.command_palette_contextual_mode = false;
        self.workspace.chrome_ui.show_radial_menu = false;
        self.workspace.graph_runtime.clip_inspector_state = Some(ClipInspectorState {
            webview_id: first.webview_id,
            source_url: first.source_url.clone(),
            page_title: first.page_title.clone(),
            search_query: String::new(),
            filter: ClipInspectorFilter::All,
            selected_index: 0,
            captures,
            pointer_stack: Vec::new(),
            pointer_stack_index: 0,
            highlight_dirty: false,
        });
        Ok(())
    }

    pub fn close_clip_inspector(&mut self) {
        self.workspace
            .graph_runtime
            .pending_clip_inspector_highlight_clear = self
            .workspace
            .graph_runtime
            .clip_inspector_state
            .as_ref()
            .map(|state| state.webview_id);
        self.workspace.chrome_ui.show_clip_inspector = false;
        self.workspace.graph_runtime.clip_inspector_state = None;
    }

    pub fn update_clip_inspector_pointer_stack(
        &mut self,
        webview_id: RendererId,
        stack: Vec<ClipCaptureData>,
    ) {
        let Some(state) = self.workspace.graph_runtime.clip_inspector_state.as_mut() else {
            return;
        };
        if state.webview_id != webview_id {
            return;
        }
        state.pointer_stack = stack;
        if state.pointer_stack_index >= state.pointer_stack.len() {
            state.pointer_stack_index = 0;
        }
        state.highlight_dirty = true;
    }

    pub fn clip_inspector_step_stack(&mut self, delta: isize) {
        let Some(state) = self.workspace.graph_runtime.clip_inspector_state.as_mut() else {
            return;
        };
        if state.pointer_stack.is_empty() {
            return;
        }
        let len = state.pointer_stack.len() as isize;
        let next = (state.pointer_stack_index as isize + delta).rem_euclid(len) as usize;
        state.pointer_stack_index = next;
        state.highlight_dirty = true;
    }

    pub fn selected_clip_inspector_stack_capture(&self) -> Option<&ClipCaptureData> {
        let state = self.workspace.graph_runtime.clip_inspector_state.as_ref()?;
        state.pointer_stack.get(state.pointer_stack_index)
    }

    pub fn clear_clip_inspector_highlight_dirty(&mut self) {
        if let Some(state) = self.workspace.graph_runtime.clip_inspector_state.as_mut() {
            state.highlight_dirty = false;
        }
    }

    fn clip_source_context(
        &self,
        webview_id: RendererId,
    ) -> Result<(NodeKey, Point2D<f32>), String> {
        let Some(source_key) = self.get_node_for_webview(webview_id) else {
            return Err("clip source is no longer mapped to a node".to_string());
        };
        let source_position = self
            .domain_graph()
            .node_projected_position(source_key)
            .unwrap_or_else(|| Point2D::new(400.0, 300.0));
        Ok((source_key, source_position))
    }

    fn create_clip_node_at_position(
        &mut self,
        source_key: NodeKey,
        capture: &ClipCaptureData,
        clip_position: Point2D<f32>,
    ) -> NodeKey {
        let clip_url = clip_data_url(build_clip_document(capture).as_str());
        let clip_key = self.add_node_and_sync(clip_url, clip_position);
        let clip_title = resolved_clip_title(capture);
        // Stage C: collect source classifications before mutating graph
        let inherited_classifications: Vec<NodeClassification> = self
            .workspace
            .domain
            .graph
            .node_classifications(source_key)
            .map(|cs| {
                cs.iter()
                    .filter(|c| {
                        matches!(
                            c.status,
                            ClassificationStatus::Accepted | ClassificationStatus::Verified
                        )
                    })
                    .map(|c| NodeClassification {
                        provenance: ClassificationProvenance::InheritedFromSource,
                        status: ClassificationStatus::Suggested,
                        primary: false,
                        ..c.clone()
                    })
                    .collect()
            })
            .unwrap_or_default();

        let graph = &mut self.workspace.domain.graph;
        let _ = graph.set_node_title(clip_key, clip_title);
        let _ = graph.insert_node_tag(clip_key, Self::TAG_CLIP.to_string());
        let _ = graph.set_node_mime_hint(clip_key, Some("text/html".to_string()));
        let _ = graph.set_node_history_state(clip_key, vec![capture.source_url.clone()], 0);
        for inherited in &inherited_classifications {
            graph.add_node_classification(clip_key, inherited.clone());
        }

        self.workspace.graph_runtime.semantic_index_dirty = true;

        // Journal inherited classifications to WAL
        if let Some(store) = &mut self.services.persistence {
            if let Some(node) = self.workspace.domain.graph.get_node(clip_key) {
                let node_id = node.id.to_string();
                for c in &inherited_classifications {
                    store.log_mutation(
                        &crate::services::persistence::types::LogEntry::AssignClassification {
                            node_id: node_id.clone(),
                            classification: c.clone(),
                        },
                    );
                }
            }
        }
        let _ = self.assert_relation_and_sync(
            source_key,
            clip_key,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                label: Some(CLIP_EDGE_LABEL.to_string()),
                decay_progress: None,
            },
        );
        clip_key
    }
}

pub fn clip_capture_matches_filter(capture: &ClipCaptureData, filter: ClipInspectorFilter) -> bool {
    match filter {
        ClipInspectorFilter::All => true,
        ClipInspectorFilter::Text => {
            !capture.text_excerpt.trim().is_empty() && capture.image_url.is_none()
        }
        ClipInspectorFilter::Link => capture.href.is_some(),
        ClipInspectorFilter::Image => {
            capture.image_url.is_some() || capture.tag_name.eq_ignore_ascii_case("img")
        }
        ClipInspectorFilter::Structure => matches!(
            capture.tag_name.as_str(),
            "article" | "section" | "aside" | "figure" | "main" | "nav" | "header" | "footer"
        ),
        ClipInspectorFilter::Media => {
            matches!(
                capture.tag_name.as_str(),
                "img" | "picture" | "video" | "audio" | "svg" | "canvas" | "figure"
            ) || capture.image_url.is_some()
        }
    }
}

pub fn clip_capture_matches_query(capture: &ClipCaptureData, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let query = query.to_ascii_lowercase();
    [
        capture.clip_title.as_str(),
        capture.text_excerpt.as_str(),
        capture.tag_name.as_str(),
        capture.href.as_deref().unwrap_or_default(),
        capture.image_url.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .any(|field| field.to_ascii_lowercase().contains(&query))
}

fn exploded_clip_position(
    source_position: Point2D<f32>,
    index: usize,
    total: usize,
) -> Point2D<f32> {
    let columns = total.clamp(1, CLIP_GRID_COLUMNS);
    let rows = total.div_ceil(columns);
    let column = index % columns;
    let row = index / columns;
    let centered_row = row as f32 - (rows.saturating_sub(1) as f32 / 2.0);
    let centered_column = column as f32 - (columns.saturating_sub(1) as f32 / 2.0);

    Point2D::new(
        source_position.x + CLIP_GRID_X_OFFSET + column as f32 * CLIP_GRID_X_SPACING,
        source_position.y + centered_row * CLIP_GRID_Y_SPACING + centered_column * 18.0,
    )
}

fn clip_data_url(document: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(document.as_bytes());
    format!("data:text/html;charset=utf-8;base64,{encoded}")
}

fn build_clip_document(capture: &ClipCaptureData) -> String {
    let escaped_title = html_escape(capture.page_title.as_deref().unwrap_or("Clip"));
    let escaped_source_url = html_escape(&capture.source_url);
    let escaped_tag_name = html_escape(&capture.tag_name);
    let escaped_excerpt = html_escape(&capture.text_excerpt);
    let escaped_href = capture.href.as_deref().map(html_escape);
    let escaped_image_url = capture.image_url.as_deref().map(html_escape);

    let mut metadata = format!(
        "<div class=\"clip-meta\"><span class=\"pill\">#clip</span><span>{escaped_tag_name}</span><span class=\"source\">{escaped_source_url}</span></div>"
    );
    if !escaped_excerpt.is_empty() {
        metadata.push_str(format!("<p class=\"excerpt\">{escaped_excerpt}</p>").as_str());
    }
    if let Some(href) = escaped_href {
        metadata.push_str(
            format!("<p class=\"link\">Link: <a href=\"{href}\">{href}</a></p>").as_str(),
        );
    }
    if let Some(image_url) = escaped_image_url {
        metadata.push_str(
            format!("<p class=\"link\">Image: <a href=\"{image_url}\">{image_url}</a></p>")
                .as_str(),
        );
    }

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{escaped_title}</title><base href=\"{escaped_source_url}\"><style>body{{margin:0;padding:24px;font-family:Georgia,serif;background:#f6f1e8;color:#1f1a17}}main{{max-width:960px;margin:0 auto;display:grid;gap:16px}}.clip-meta{{display:flex;flex-wrap:wrap;gap:8px;align-items:center;font:12px/1.4 monospace;color:#5b4c41}}.pill{{display:inline-block;padding:2px 8px;border:1px dashed #8c6f57;border-radius:999px;background:#fff7ea}}.source{{max-width:100%;overflow-wrap:anywhere}}.excerpt,.link{{margin:0;font-size:13px;color:#5b4c41;overflow-wrap:anywhere}}.clip-frame{{padding:18px;border:1px solid #d8c5ad;border-radius:18px;background:#fffaf2;box-shadow:0 10px 30px rgba(54,35,20,0.08)}}img{{max-width:100%;height:auto}}</style></head><body><main>{metadata}<section class=\"clip-frame\">{}</section></main></body></html>",
        capture.outer_html
    )
}

fn resolved_clip_title(capture: &ClipCaptureData) -> String {
    let candidate = capture.clip_title.trim();
    if !candidate.is_empty() {
        return truncate(candidate, CLIP_TEXT_LIMIT);
    }
    let excerpt = capture.text_excerpt.trim();
    if !excerpt.is_empty() {
        return truncate(excerpt, CLIP_TEXT_LIMIT);
    }
    if !capture.tag_name.trim().is_empty() {
        return format!("Clip: <{}>", capture.tag_name.trim().to_ascii_lowercase());
    }
    CLIP_TITLE_FALLBACK.to_string()
}

fn truncate(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use euclid::default::Point2D;
    use servo::WebViewId;

    use super::{
        CLIP_EDGE_LABEL, ClipCaptureData, build_clip_document, exploded_clip_position,
        resolved_clip_title,
    };
    use crate::app::GraphBrowserApp;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn resolved_clip_title_prefers_explicit_title() {
        let title = resolved_clip_title(&ClipCaptureData {
            webview_id: test_webview_id(),
            source_url: "https://example.com".to_string(),
            page_title: Some("Example".to_string()),
            clip_title: "Primary heading".to_string(),
            outer_html: "<h1>Primary heading</h1>".to_string(),
            text_excerpt: "Fallback".to_string(),
            tag_name: "h1".to_string(),
            href: None,
            image_url: None,
            dom_path: None,
        });
        assert_eq!(title, "Primary heading");
    }

    #[test]
    fn build_clip_document_includes_base_and_metadata() {
        let document = build_clip_document(&ClipCaptureData {
            webview_id: test_webview_id(),
            source_url: "https://example.com/article".to_string(),
            page_title: Some("Example Article".to_string()),
            clip_title: "Example".to_string(),
            outer_html: "<section><p>Hello</p></section>".to_string(),
            text_excerpt: "Hello".to_string(),
            tag_name: "section".to_string(),
            href: Some("https://example.com/link".to_string()),
            image_url: None,
            dom_path: None,
        });

        assert!(document.contains("<base href=\"https://example.com/article\">"));
        assert!(document.contains("#clip"));
        assert!(document.contains("<section><p>Hello</p></section>"));
        assert!(document.contains("https://example.com/link"));
    }

    #[test]
    fn create_clip_node_from_capture_tags_and_links_new_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source_key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(10.0, 20.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, source_key);

        let clip_key = app
            .create_clip_node_from_capture(&ClipCaptureData {
                webview_id,
                source_url: "https://example.com".to_string(),
                page_title: Some("Example".to_string()),
                clip_title: "Card".to_string(),
                outer_html: "<article><h2>Card</h2></article>".to_string(),
                text_excerpt: "Card".to_string(),
                tag_name: "article".to_string(),
                href: None,
                image_url: None,
                dom_path: None,
            })
            .expect("clip capture should create a node");

        let clip_node = app
            .workspace
            .domain
            .graph
            .get_node(clip_key)
            .expect("clip node should exist");
        assert!(clip_node.tags.contains(GraphBrowserApp::TAG_CLIP));
        assert_eq!(clip_node.title, "Card");
        assert_eq!(
            clip_node.history_entries,
            vec!["https://example.com".to_string()]
        );
        assert!(clip_node.url().starts_with("data:text/html"));

        let has_clip_edge = app
            .workspace
            .domain
            .graph
            .find_edge_key(source_key, clip_key)
            .and_then(|edge_key| app.workspace.domain.graph.get_edge(edge_key))
            .is_some_and(|payload| {
                payload.has_relation(crate::graph::RelationSelector::Semantic(
                    crate::graph::SemanticSubKind::UserGrouped,
                )) && payload.label() == Some(CLIP_EDGE_LABEL)
            });
        assert!(has_clip_edge, "expected labeled clip-source edge");
    }

    #[test]
    fn exploded_clip_position_fans_batches_into_rows() {
        let source = Point2D::new(100.0, 100.0);
        let first = exploded_clip_position(source, 0, 5);
        let third = exploded_clip_position(source, 2, 5);
        let fourth = exploded_clip_position(source, 3, 5);

        assert!(first.x > source.x);
        assert!(third.x > first.x);
        assert!(fourth.y > first.y);
    }

    #[test]
    fn create_clip_nodes_from_captures_creates_multiple_linked_clips() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source_key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(10.0, 20.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, source_key);

        let clip_keys = app
            .create_clip_nodes_from_captures(&[
                ClipCaptureData {
                    webview_id,
                    source_url: "https://example.com".to_string(),
                    page_title: Some("Example".to_string()),
                    clip_title: "Hero".to_string(),
                    outer_html: "<section><h1>Hero</h1></section>".to_string(),
                    text_excerpt: "Hero".to_string(),
                    tag_name: "section".to_string(),
                    href: None,
                    image_url: None,
                    dom_path: None,
                },
                ClipCaptureData {
                    webview_id,
                    source_url: "https://example.com".to_string(),
                    page_title: Some("Example".to_string()),
                    clip_title: "Card".to_string(),
                    outer_html: "<article><h2>Card</h2></article>".to_string(),
                    text_excerpt: "Card".to_string(),
                    tag_name: "article".to_string(),
                    href: None,
                    image_url: None,
                    dom_path: None,
                },
            ])
            .expect("clip batch should create nodes");

        assert_eq!(clip_keys.len(), 2);
        for clip_key in &clip_keys {
            let clip_node = app
                .workspace
                .domain
                .graph
                .get_node(*clip_key)
                .expect("clip node should exist");
            assert!(clip_node.tags.contains(GraphBrowserApp::TAG_CLIP));
        }
        let clip_edges = app
            .workspace
            .domain
            .graph
            .edges()
            .filter(|edge| edge.from == source_key && clip_keys.contains(&edge.to))
            .count();
        assert_eq!(clip_edges, 2);
    }

    #[test]
    fn clip_node_inherits_source_classifications_with_inherited_provenance() {
        // Spec: graph_enrichment_plan.md §Stage C done gate —
        // "at least one end-to-end import or clip path produces visible enrichment"
        // "inherited metadata is marked with provenance"
        use crate::app::GraphIntent;
        use crate::model::graph::{
            ClassificationProvenance, ClassificationScheme, ClassificationStatus,
            NodeClassification,
        };

        let mut app = GraphBrowserApp::new_for_testing();
        let source_key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(10.0, 20.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, source_key);

        // Give the source an accepted classification
        app.apply_reducer_intents([GraphIntent::AssignClassification {
            key: source_key,
            classification: NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:51".to_string(),
                label: Some("Mathematics".to_string()),
                confidence: 1.0,
                provenance: ClassificationProvenance::UserAuthored,
                status: ClassificationStatus::Accepted,
                primary: true,
            },
        }]);

        let clip_key = app
            .create_clip_node_from_capture(&ClipCaptureData {
                webview_id,
                source_url: "https://example.com".to_string(),
                page_title: Some("Example".to_string()),
                clip_title: "Section".to_string(),
                outer_html: "<section><p>text</p></section>".to_string(),
                text_excerpt: "text".to_string(),
                tag_name: "section".to_string(),
                href: None,
                image_url: None,
                dom_path: None,
            })
            .expect("clip capture should succeed");

        let classifications = app
            .workspace
            .domain
            .graph
            .node_classifications(clip_key)
            .expect("clip node should exist");

        assert_eq!(
            classifications.len(),
            1,
            "clip should inherit one classification"
        );
        let c = &classifications[0];
        assert_eq!(c.value, "udc:51");
        assert_eq!(
            c.provenance,
            ClassificationProvenance::InheritedFromSource,
            "inherited classification must carry InheritedFromSource provenance"
        );
        assert_eq!(
            c.status,
            ClassificationStatus::Suggested,
            "inherited classification must be Suggested (not auto-accepted)"
        );
        assert!(!c.primary, "inherited classification must not be primary");
    }
}
