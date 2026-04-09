/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::middlenet::adapters::{parse_feed, parse_gophermap, parse_markdown, parse_plain_text};
use crate::middlenet::source::{MiddleNetContent, MiddleNetContentKind, MiddleNetSource};
use crate::middlenet::transport::fetch_remote_text;

#[derive(Debug, Clone)]
pub(crate) enum MiddleNetLoadResult {
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

pub(crate) struct MiddleNetEngine;

impl MiddleNetEngine {
    pub(crate) fn parse_text(source: MiddleNetSource, body: &str) -> MiddleNetLoadResult {
        let (document, title_hint) = match source.content_kind {
            MiddleNetContentKind::GeminiText => (
                crate::middlenet::document::SimpleDocument::from_gemini(body),
                None,
            ),
            MiddleNetContentKind::GopherMap => (parse_gophermap(body), None),
            MiddleNetContentKind::FingerText | MiddleNetContentKind::PlainText => {
                (parse_plain_text(body), None)
            }
            MiddleNetContentKind::Markdown => (parse_markdown(body), None),
            MiddleNetContentKind::Rss | MiddleNetContentKind::Atom => {
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

    pub(crate) fn load_remote(source: MiddleNetSource) -> MiddleNetLoadResult {
        match fetch_remote_text(&source) {
            Ok(fetch) => {
                let mut parsed_source = source;
                if let Some(content_kind) = fetch.content_kind_override {
                    parsed_source.content_kind = content_kind;
                }
                Self::parse_text(parsed_source, &fetch.body)
            }
            Err(error) => {
                if matches!(
                    source.canonical_uri.as_deref().and_then(uri_scheme),
                    Some("misfin")
                ) {
                    return MiddleNetLoadResult::TransportPending {
                        source,
                        note: error,
                    };
                }

                MiddleNetLoadResult::TransportError { source, error }
            }
        }
    }

    pub(crate) fn transport_pending(source: MiddleNetSource) -> MiddleNetLoadResult {
        MiddleNetLoadResult::TransportPending {
            source,
            note: "Protocol recognition and viewer routing are in place, but transport fetching is not wired into the MiddleNet engine yet."
                .to_string(),
        }
    }
}

fn uri_scheme(uri: &str) -> Option<&str> {
    uri.split_once(':').map(|(scheme, _)| scheme)
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

    #[test]
    fn remote_misfin_stays_pending_until_transport_exists() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText)
            .with_uri("misfin://example.com/");

        let result = MiddleNetEngine::load_remote(source);

        assert!(matches!(result, MiddleNetLoadResult::TransportPending { .. }));
    }
}