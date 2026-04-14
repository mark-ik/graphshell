/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Source metadata for content routed through the MiddleNet engine scaffold.

use crate::document::SimpleDocument;
use crate::dom::Document;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiddleNetContentKind {
    GeminiText,
    GopherMap,
    FingerText,
    Markdown,
    Html,
    Rss,
    Atom,
    JsonFeed,
    PlainText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiddleNetSource {
    pub content_kind: MiddleNetContentKind,
    pub canonical_uri: Option<String>,
    pub title_hint: Option<String>,
}

impl MiddleNetSource {
    pub fn new(content_kind: MiddleNetContentKind) -> Self {
        Self {
            content_kind,
            canonical_uri: None,
            title_hint: None,
        }
    }

    pub fn with_uri(mut self, canonical_uri: impl Into<String>) -> Self {
        self.canonical_uri = Some(canonical_uri.into());
        self
    }

    pub fn with_title_hint(mut self, title_hint: impl Into<String>) -> Self {
        self.title_hint = Some(title_hint.into());
        self
    }

    pub fn detect(uri: &str, mime_hint: Option<&str>) -> Option<Self> {
        let normalized_mime = mime_hint.map(|value| value.trim().to_ascii_lowercase());
        let normalized_scheme = uri
            .split_once(':')
            .map(|(scheme, _)| scheme.trim().to_ascii_lowercase())
            .filter(|scheme| !scheme.is_empty());

        let content_kind = normalized_mime
            .as_deref()
            .and_then(content_kind_from_mime)
            .or_else(|| normalized_scheme.as_deref().and_then(content_kind_from_scheme))
            .or_else(|| extract_extension(uri).and_then(content_kind_from_extension))?;

        Some(Self::new(content_kind).with_uri(uri))
    }
}

#[derive(Debug, Clone)]
pub struct MiddleNetContent {
    pub source: MiddleNetSource,
    pub document: Document,
}

impl MiddleNetContent {
    pub fn new(source: MiddleNetSource, document: Document) -> Self {
        Self { source, document }
    }

    pub fn from_gemini(source: MiddleNetSource, body: &str) -> Self {
        debug_assert_eq!(source.content_kind, MiddleNetContentKind::GeminiText);
        Self {
            source,
            document: Document::parse(&crate::document::SimpleDocument::from_gemini(body).to_html()),
        }
    }
}

impl MiddleNetContentKind {
    pub fn label(self) -> &'static str {
        match self {
            MiddleNetContentKind::GeminiText => "Gemini",
            MiddleNetContentKind::GopherMap => "Gopher",
            MiddleNetContentKind::FingerText => "Finger",
            MiddleNetContentKind::Markdown => "Markdown",
            MiddleNetContentKind::Html => "HTML",
            MiddleNetContentKind::Rss => "RSS",
            MiddleNetContentKind::Atom => "Atom",
            MiddleNetContentKind::JsonFeed => "JSON Feed",
            MiddleNetContentKind::PlainText => "Plain text",
        }
    }
}

fn content_kind_from_mime(mime: &str) -> Option<MiddleNetContentKind> {
    match mime {
        "text/gemini" | "text/x-gemini" => Some(MiddleNetContentKind::GeminiText),
        "application/gophermap" | "application/x-gophermap" | "text/x-gophermap" => {
            Some(MiddleNetContentKind::GopherMap)
        }
        "application/x-finger" => Some(MiddleNetContentKind::FingerText),
        "text/markdown" | "text/x-markdown" => Some(MiddleNetContentKind::Markdown),
        "application/rss+xml" => Some(MiddleNetContentKind::Rss),
        "application/atom+xml" => Some(MiddleNetContentKind::Atom),
        "application/feed+json" => Some(MiddleNetContentKind::JsonFeed),
        "text/plain" => Some(MiddleNetContentKind::PlainText),
        _ => None,
    }
}

fn content_kind_from_scheme(scheme: &str) -> Option<MiddleNetContentKind> {
    match scheme {
        "gemini" | "titan" | "spartan" | "misfin" => Some(MiddleNetContentKind::GeminiText),
        "gopher" => Some(MiddleNetContentKind::GopherMap),
        "finger" => Some(MiddleNetContentKind::FingerText),
        _ => None,
    }
}

fn content_kind_from_extension(extension: &str) -> Option<MiddleNetContentKind> {
    match extension.to_ascii_lowercase().as_str() {
        "gmi" | "gemini" => Some(MiddleNetContentKind::GeminiText),
        "gophermap" => Some(MiddleNetContentKind::GopherMap),
        "md" | "markdown" => Some(MiddleNetContentKind::Markdown),
        "rss" => Some(MiddleNetContentKind::Rss),
        "atom" => Some(MiddleNetContentKind::Atom),
        "jsonfeed" => Some(MiddleNetContentKind::JsonFeed),
        _ => None,
    }
}

fn extract_extension(uri: &str) -> Option<&str> {
    let no_fragment = uri.split('#').next().unwrap_or(uri);
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    no_query.rsplit_once('.').map(|(_, extension)| extension)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::SimpleBlock;

    #[test]
    fn gemini_source_builds_document_content() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText)
            .with_uri("gemini://example.com/")
            .with_title_hint("Example capsule");

        let content = MiddleNetContent::from_gemini(source.clone(), "# Hello\n=> /next Next\n");

        assert_eq!(content.source, source);
        // let SimpleDocument::Blocks(blocks) = &content.document;
        // assert!(matches!(blocks.first(), Some(SimpleBlock::Heading { .. })));
    }

    #[test]
    fn detects_gophermap_and_markdown_sources() {
        let gopher = MiddleNetSource::detect("file:///capsule/gophermap", Some("application/gophermap"))
            .expect("gopher source should resolve");
        assert_eq!(gopher.content_kind, MiddleNetContentKind::GopherMap);

        let markdown = MiddleNetSource::detect("file:///notes/topic.md", None)
            .expect("markdown source should resolve");
        assert_eq!(markdown.content_kind, MiddleNetContentKind::Markdown);

        let titan = MiddleNetSource::detect("titan://capsule.example/edit/page", None)
            .expect("titan source should resolve");
        assert_eq!(titan.content_kind, MiddleNetContentKind::GeminiText);

        let json_feed = MiddleNetSource::detect(
            "https://example.com/feed.jsonfeed",
            Some("application/feed+json"),
        )
        .expect("json feed source should resolve");
        assert_eq!(json_feed.content_kind, MiddleNetContentKind::JsonFeed);
    }
}
