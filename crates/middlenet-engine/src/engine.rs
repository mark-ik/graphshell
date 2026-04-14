/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::adapters::{parse_feed, parse_gophermap, parse_markdown, parse_plain_text};
use crate::source::{MiddleNetContent, MiddleNetContentKind, MiddleNetSource};

#[derive(Debug, Clone)]
pub enum MiddleNetLoadResult {
    Parsed(MiddleNetContent),
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
    pub fn parse_text(source: MiddleNetSource, body: &str) -> MiddleNetLoadResult {
        let (document, title_hint) = match source.content_kind {
            MiddleNetContentKind::GeminiText => (
                crate::dom::Document::parse(&crate::document::SimpleDocument::from_gemini(body).to_html()),
                None,
            ),
            MiddleNetContentKind::GopherMap => (parse_gophermap(body), None),
            MiddleNetContentKind::FingerText | MiddleNetContentKind::PlainText => {
                (parse_plain_text(body), None)
            }
            MiddleNetContentKind::Markdown => (parse_markdown(body), None),
            MiddleNetContentKind::Rss
            | MiddleNetContentKind::Atom
            | MiddleNetContentKind::JsonFeed => {
                match parse_feed(source.content_kind, body) {
                    Ok((document, title_hint)) => (document, title_hint),
                    Err(error) => {
                        return MiddleNetLoadResult::ParseError { source, error };
                    }
                }
            }
            MiddleNetContentKind::Html => {
                return MiddleNetLoadResult::Unsupported {
                    source,
                    note: "HTML adaptation is still delegated to the existing web viewers."
                        .to_string(),
                };
            }
        };

        let mut parsed_source = source;
        if parsed_source.title_hint.is_none() {
            parsed_source.title_hint = title_hint;
        }

        MiddleNetLoadResult::Parsed(MiddleNetContent::new(parsed_source, document))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_source_parses_into_document() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown)
            .with_uri("file:///notes/topic.md");

        let result = MiddleNetEngine::parse_text(source, "# Topic\n- item\n");

        assert!(matches!(result, MiddleNetLoadResult::Parsed(_)));
    }
}


