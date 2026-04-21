/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use middlenet_core::document::{
    DocumentDiagnostics, DocumentMeta, DocumentProvenance, DocumentTrustState, SemanticBlock,
    SemanticDocument,
};
use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};
use middlenet_render::{RenderRequest, RenderScene};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptContext {
    pub fetched_at: Option<String>,
    pub trust_state: Option<DocumentTrustState>,
    pub source_label: Option<String>,
    pub retain_raw_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptationMetadata {
    pub body_len: usize,
    pub streamed: bool,
    pub raw_source_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreparedDocument {
    pub source: MiddleNetSource,
    pub document: SemanticDocument,
    pub provenance: DocumentProvenance,
    pub diagnostics: DocumentDiagnostics,
    pub trust_state: DocumentTrustState,
    pub adaptation: AdaptationMetadata,
    pub raw_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneDecision {
    Direct,
    Html,
    FaithfulSource,
    Unsupported,
}

impl LaneDecision {
    pub fn label(self) -> &'static str {
        match self {
            Self::Direct => "Direct",
            Self::Html => "HTML",
            Self::FaithfulSource => "Faithful Source",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneOverride {
    Direct,
    Html,
    FaithfulSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    pub supports_direct_lane: bool,
    pub supports_html_lane: bool,
    pub supports_faithful_source_lane: bool,
}

impl Default for HostCapabilities {
    fn default() -> Self {
        Self {
            supports_direct_lane: true,
            supports_html_lane: false,
            supports_faithful_source_lane: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneRenderOutput {
    pub lane: LaneDecision,
    pub scene: Option<RenderScene>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MiddleNetLoadResult {
    Parsed(PreparedDocument),
    TransportPending {
        source: MiddleNetSource,
        note: String,
    },
    TransportError {
        source: MiddleNetSource,
        error: String,
    },
    Unsupported {
        source: MiddleNetSource,
        note: String,
    },
    ParseError {
        source: MiddleNetSource,
        error: String,
    },
}

pub struct MiddleNetEngine;

impl MiddleNetEngine {
    pub fn detect_source(uri: &str, mime_hint: Option<&str>) -> Option<MiddleNetSource> {
        MiddleNetSource::detect(uri, mime_hint)
    }

    pub fn adapt(
        source: MiddleNetSource,
        body: &str,
        context: AdaptContext,
    ) -> Result<PreparedDocument, String> {
        let mut document = middlenet_adapters::adapt(&source, body)?;
        patch_document_meta(&source, &mut document, &context);
        let provenance = document.provenance.clone();
        let diagnostics = document.meta.diagnostics.clone();
        let trust_state = document.meta.trust_state.clone();
        Ok(PreparedDocument {
            source,
            document,
            provenance,
            diagnostics,
            trust_state,
            adaptation: AdaptationMetadata {
                body_len: body.len(),
                streamed: false,
                raw_source_available: true,
            },
            raw_source: context.retain_raw_source.then(|| body.to_string()),
        })
    }

    pub fn parse_text(source: MiddleNetSource, body: &str) -> MiddleNetLoadResult {
        if matches!(source.content_kind, MiddleNetContentKind::Html) {
            return MiddleNetLoadResult::Unsupported {
                source,
                note: "HTML adaptation is still delegated to the existing web viewers.".to_string(),
            };
        }

        match Self::adapt(
            source.clone(),
            body,
            AdaptContext {
                retain_raw_source: true,
                ..AdaptContext::default()
            },
        ) {
            Ok(prepared) => MiddleNetLoadResult::Parsed(prepared),
            Err(error) => MiddleNetLoadResult::ParseError { source, error },
        }
    }

    pub fn select_lane(
        prepared: &PreparedDocument,
        host_caps: &HostCapabilities,
        override_lane: Option<LaneOverride>,
    ) -> LaneDecision {
        Self::select_lane_for_source(&prepared.source, host_caps, override_lane)
    }

    pub fn select_lane_for_source(
        source: &MiddleNetSource,
        host_caps: &HostCapabilities,
        override_lane: Option<LaneOverride>,
    ) -> LaneDecision {
        if let Some(override_lane) = override_lane {
            let forced = match override_lane {
                LaneOverride::Direct if host_caps.supports_direct_lane => LaneDecision::Direct,
                LaneOverride::Html if host_caps.supports_html_lane => LaneDecision::Html,
                LaneOverride::FaithfulSource if host_caps.supports_faithful_source_lane => {
                    LaneDecision::FaithfulSource
                }
                _ => LaneDecision::Unsupported,
            };
            if forced != LaneDecision::Unsupported {
                return forced;
            }
        }

        match source.content_kind {
            MiddleNetContentKind::GeminiText
            | MiddleNetContentKind::GopherMap
            | MiddleNetContentKind::FingerText
            | MiddleNetContentKind::Markdown
            | MiddleNetContentKind::PlainText
            | MiddleNetContentKind::Rss
            | MiddleNetContentKind::Atom
            | MiddleNetContentKind::JsonFeed
                if host_caps.supports_direct_lane =>
            {
                LaneDecision::Direct
            }
            MiddleNetContentKind::Html if host_caps.supports_html_lane => LaneDecision::Html,
            _ if host_caps.supports_faithful_source_lane => LaneDecision::FaithfulSource,
            _ => LaneDecision::Unsupported,
        }
    }

    pub fn render(
        prepared: &PreparedDocument,
        lane_decision: LaneDecision,
        request: &RenderRequest,
    ) -> LaneRenderOutput {
        match lane_decision {
            LaneDecision::Direct => LaneRenderOutput {
                lane: lane_decision,
                scene: Some(middlenet_render::render_document(
                    &prepared.document,
                    request,
                )),
                note: None,
            },
            LaneDecision::Html => LaneRenderOutput {
                lane: lane_decision,
                scene: None,
                note: Some("HTML lane is not implemented yet for Middlenet.".to_string()),
            },
            LaneDecision::FaithfulSource => LaneRenderOutput {
                lane: lane_decision,
                scene: Some(middlenet_render::render_document(
                    &faithful_source_document(prepared),
                    request,
                )),
                note: None,
            },
            LaneDecision::Unsupported => LaneRenderOutput {
                lane: lane_decision,
                scene: None,
                note: Some("No Middlenet-capable lane matched this source.".to_string()),
            },
        }
    }
}

fn faithful_source_document(prepared: &PreparedDocument) -> SemanticDocument {
    let mut document = SemanticDocument::empty_for_source(&prepared.source);
    document.meta.title = prepared
        .document
        .meta
        .title
        .clone()
        .or_else(|| Some("Faithful source".to_string()));
    document.meta.subtitle = prepared.document.meta.subtitle.clone();
    document.meta.source_label = prepared.document.meta.source_label.clone();
    document.meta.fetched_at = prepared.document.meta.fetched_at.clone();
    document.meta.trust_state = prepared.trust_state.clone();
    document.meta.diagnostics = prepared.diagnostics.clone();
    document.meta.alternate_open_targets = prepared.document.meta.alternate_open_targets.clone();
    document.meta.raw_source_available = prepared.raw_source.is_some();
    document.meta.article_hint = prepared.document.meta.article_hint.clone();
    document.meta.feed_hint = prepared.document.meta.feed_hint.clone();
    document.provenance = prepared.provenance.clone();

    let mut blocks = vec![
        SemanticBlock::RawSourceNotice {
            note: "Rendering the faithful source view because no richer Middlenet lane matched."
                .to_string(),
        },
        SemanticBlock::MetadataRow {
            label: "Content kind".to_string(),
            value: prepared.source.content_kind.label().to_string(),
        },
    ];

    if let Some(uri) = prepared.source.canonical_uri.as_deref() {
        blocks.push(SemanticBlock::MetadataRow {
            label: "Source".to_string(),
            value: uri.to_string(),
        });
    }

    match prepared.raw_source.as_deref() {
        Some(raw_source) => blocks.push(SemanticBlock::CodeFence {
            lang: None,
            text: raw_source.to_string(),
        }),
        None => blocks.push(SemanticBlock::Paragraph(
            "Raw source is not available for this document.".to_string(),
        )),
    }

    document.blocks = blocks;
    document
}

fn patch_document_meta(
    source: &MiddleNetSource,
    document: &mut SemanticDocument,
    context: &AdaptContext,
) {
    let mut meta = DocumentMeta::for_source(source);
    meta.title = document
        .meta
        .title
        .clone()
        .or_else(|| source.title_hint.clone())
        .or_else(|| document.first_title().map(ToOwned::to_owned));
    meta.subtitle = document.meta.subtitle.clone();
    meta.trust_state = context
        .trust_state
        .clone()
        .unwrap_or_else(|| document.meta.trust_state.clone());
    meta.diagnostics = document.meta.diagnostics.clone();
    meta.alternate_open_targets = document.meta.alternate_open_targets.clone();
    meta.raw_source_available = true;
    meta.article_hint = document.meta.article_hint.clone();
    meta.feed_hint = document.meta.feed_hint.clone();
    if let Some(fetched_at) = context.fetched_at.clone() {
        meta.fetched_at = Some(fetched_at);
    } else {
        meta.fetched_at = document.meta.fetched_at.clone();
    }
    if let Some(label) = context.source_label.clone() {
        meta.source_label = Some(label);
    }

    document.meta = meta;
    document.provenance = DocumentProvenance {
        source_kind: source.content_kind,
        canonical_uri: source.canonical_uri.clone(),
        fetched_at: context
            .fetched_at
            .clone()
            .or_else(|| document.provenance.fetched_at.clone()),
        source_label: context
            .source_label
            .clone()
            .or_else(|| document.provenance.source_label.clone()),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_render::RenderMode;

    #[test]
    fn markdown_source_parses_into_prepared_document() {
        let source =
            MiddleNetSource::new(MiddleNetContentKind::Markdown).with_uri("file:///notes/topic.md");

        let result = MiddleNetEngine::parse_text(source, "# Topic\n- item\n");

        assert!(matches!(result, MiddleNetLoadResult::Parsed(_)));
    }

    #[test]
    fn feeds_choose_direct_lane() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Rss)
            .with_uri("https://example.com/feed.xml");
        let prepared = MiddleNetEngine::adapt(
            source,
            r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Feed</title></channel></rss>"#,
            AdaptContext::default(),
        )
        .expect("feed should adapt");

        assert_eq!(
            MiddleNetEngine::select_lane(&prepared, &HostCapabilities::default(), None),
            LaneDecision::Direct
        );
    }

    #[test]
    fn direct_lane_renders_scene() {
        let source = MiddleNetSource::new(MiddleNetContentKind::PlainText)
            .with_uri("file:///notes/topic.txt");
        let prepared =
            MiddleNetEngine::adapt(source, "Hello from Middlenet", AdaptContext::default())
                .expect("plain text should adapt");

        let output = MiddleNetEngine::render(
            &prepared,
            LaneDecision::Direct,
            &RenderRequest {
                mode: RenderMode::Card,
                ..RenderRequest::default()
            },
        );

        assert_eq!(output.lane, LaneDecision::Direct);
        assert!(output.scene.is_some());
    }

    #[test]
    fn html_without_html_lane_falls_back_to_faithful_source() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Html)
            .with_uri("https://example.com/index.html");

        let lane = MiddleNetEngine::select_lane_for_source(&source, &HostCapabilities::default(), None);

        assert_eq!(lane, LaneDecision::FaithfulSource);
    }
}
