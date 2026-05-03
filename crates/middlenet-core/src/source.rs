/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use serde::{Deserialize, Serialize};

use crate::document::{DocumentDiagnostics, DocumentMeta, SemanticDocument};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Spartan protocol (spartan://) — gemini-shaped text with a
    /// simpler, prompt-driven request/response. See
    /// `crates/middlenet-spartan` (Slice 61 scaffold).
    SpartanText,
    /// Nex protocol (nex://) — minimal small-web protocol: directory
    /// listings + plain text. See `crates/middlenet-nex` (Slice 61
    /// scaffold).
    NexDirectory,
    /// Titan protocol (titan://) — write companion to gemini.
    /// Submission body is gemtext-shaped; the request envelope is
    /// titan-specific. See `crates/middlenet-titan` (Slice 61
    /// scaffold).
    TitanWrite,
    /// Scroll protocol — newer small-web protocol with binary
    /// metadata + text. See `crates/middlenet-scroll` (Slice 61
    /// scaffold).
    ScrollDocument,
    /// Guppy protocol — UDP-based small-web protocol with chunked
    /// request/response. See `crates/middlenet-guppy` (Slice 61
    /// scaffold).
    GuppyText,
    /// Misfin (misfin://) — gemini-shaped peer-to-peer email.
    /// Body is gemtext; envelope carries sender/recipient/timestamp.
    /// See `crates/middlenet-misfin` (Slice 61 scaffold; eventual
    /// real impl wires through `graphshell-comms::misfin`).
    MisfinMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            .or_else(|| {
                normalized_scheme
                    .as_deref()
                    .and_then(content_kind_from_scheme)
            })
            .or_else(|| extract_extension(uri).and_then(content_kind_from_extension))?;

        Some(Self::new(content_kind).with_uri(uri))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MiddleNetContent {
    pub source: MiddleNetSource,
    pub document: SemanticDocument,
}

impl MiddleNetContent {
    pub fn new(source: MiddleNetSource, document: SemanticDocument) -> Self {
        Self { source, document }
    }

    pub fn from_gemini(source: MiddleNetSource, body: &str) -> Self {
        debug_assert_eq!(source.content_kind, MiddleNetContentKind::GeminiText);
        let mut document = SemanticDocument::from_gemini(body);
        document.meta = DocumentMeta::for_source(&source);
        document.meta.title = source
            .title_hint
            .clone()
            .or_else(|| document.first_title().map(ToOwned::to_owned));
        document.meta.diagnostics = DocumentDiagnostics::default();
        Self { source, document }
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
            MiddleNetContentKind::SpartanText => "Spartan",
            MiddleNetContentKind::NexDirectory => "Nex",
            MiddleNetContentKind::TitanWrite => "Titan",
            MiddleNetContentKind::ScrollDocument => "Scroll",
            MiddleNetContentKind::GuppyText => "Guppy",
            MiddleNetContentKind::MisfinMessage => "Misfin",
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
        "gemini" => Some(MiddleNetContentKind::GeminiText),
        "gopher" => Some(MiddleNetContentKind::GopherMap),
        "finger" => Some(MiddleNetContentKind::FingerText),
        "spartan" => Some(MiddleNetContentKind::SpartanText),
        "nex" => Some(MiddleNetContentKind::NexDirectory),
        "titan" => Some(MiddleNetContentKind::TitanWrite),
        "scroll" => Some(MiddleNetContentKind::ScrollDocument),
        "guppy" => Some(MiddleNetContentKind::GuppyText),
        "misfin" => Some(MiddleNetContentKind::MisfinMessage),
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

    #[test]
    fn gemini_source_builds_semantic_content() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText)
            .with_uri("gemini://example.com/")
            .with_title_hint("Example capsule");

        let content = MiddleNetContent::from_gemini(source.clone(), "# Hello\n=> /next Next\n");

        assert_eq!(content.source, source);
        assert_eq!(
            content.document.meta.title.as_deref(),
            Some("Example capsule")
        );
    }

    #[test]
    fn detects_gophermap_and_markdown_sources() {
        let gopher =
            MiddleNetSource::detect("file:///capsule/gophermap", Some("application/gophermap"))
                .expect("gopher source should resolve");
        assert_eq!(gopher.content_kind, MiddleNetContentKind::GopherMap);

        let markdown = MiddleNetSource::detect("file:///notes/topic.md", None)
            .expect("markdown source should resolve");
        assert_eq!(markdown.content_kind, MiddleNetContentKind::Markdown);

        // Slice 61: titan resolves to its own content kind now that
        // middlenet-titan exists as a scaffold crate. Pre-Slice-61
        // it was lumped under GeminiText.
        let titan = MiddleNetSource::detect("titan://capsule.example/edit/page", None)
            .expect("titan source should resolve");
        assert_eq!(titan.content_kind, MiddleNetContentKind::TitanWrite);

        let json_feed = MiddleNetSource::detect(
            "https://example.com/feed.jsonfeed",
            Some("application/feed+json"),
        )
        .expect("json feed source should resolve");
        assert_eq!(json_feed.content_kind, MiddleNetContentKind::JsonFeed);
    }
}
