/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Middlenet adapter dispatcher — routes a [`MiddleNetSource`] +
//! body to the appropriate per-protocol parser. The parsers
//! themselves live in their own crates per the Slice 49b template:
//!
//! Real parsers:
//! - [`middlenet_gemini::parse_gemini`] (Slice 61)
//! - [`middlenet_gopher::parse_gophermap`] (Slice 49b)
//! - [`middlenet_finger::parse_finger`] (Slice 61)
//! - [`middlenet_markdown::parse_markdown`] (Slice 60)
//! - [`middlenet_plain_text::parse_plain_text`] (Slice 60)
//! - [`middlenet_feed::parse_feed`] (Slice 60 — RSS / Atom / JSON Feed)
//!
//! Scaffolds (return `Err` for now; crates exist to reserve the
//! namespace and hold the parser signature):
//! - [`middlenet_spartan::parse_spartan`] (Slice 61)
//! - [`middlenet_nex::parse_nex`] (Slice 61)
//! - [`middlenet_titan::parse_titan`] (Slice 61)
//! - [`middlenet_scroll::parse_scroll`] (Slice 61)
//! - [`middlenet_guppy::parse_guppy`] (Slice 61)
//! - [`middlenet_misfin::parse_misfin`] (Slice 61)
//!
//! This crate keeps the dispatcher (`adapt`, `adapt_streaming`) and
//! re-exports each parser so existing call sites that import via
//! `middlenet_adapters::*` continue to work. New code should depend
//! on the per-protocol crate it actually needs.

use middlenet_core::document::{DocumentDelta, SemanticDocument};
use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};

pub use middlenet_feed::parse_feed;
pub use middlenet_finger::parse_finger;
pub use middlenet_gemini::{parse_gemini, serialize_gemini};
pub use middlenet_gopher::parse_gophermap;
pub use middlenet_guppy::parse_guppy;
pub use middlenet_markdown::parse_markdown;
pub use middlenet_misfin::parse_misfin;
pub use middlenet_nex::parse_nex;
pub use middlenet_plain_text::parse_plain_text;
pub use middlenet_scroll::parse_scroll;
pub use middlenet_spartan::parse_spartan;
pub use middlenet_titan::parse_titan;

pub fn adapt(source: &MiddleNetSource, body: &str) -> Result<SemanticDocument, String> {
    match source.content_kind {
        MiddleNetContentKind::GeminiText => Ok(parse_gemini(source, body)),
        MiddleNetContentKind::GopherMap => Ok(parse_gophermap(source, body)),
        MiddleNetContentKind::FingerText => Ok(parse_finger(source, body)),
        MiddleNetContentKind::PlainText => Ok(parse_plain_text(source, body)),
        MiddleNetContentKind::Markdown => Ok(parse_markdown(source, body)),
        MiddleNetContentKind::Rss | MiddleNetContentKind::Atom | MiddleNetContentKind::JsonFeed => {
            parse_feed(source, body)
        }
        MiddleNetContentKind::SpartanText => parse_spartan(source, body),
        MiddleNetContentKind::NexDirectory => parse_nex(source, body),
        MiddleNetContentKind::TitanWrite => parse_titan(source, body),
        MiddleNetContentKind::ScrollDocument => parse_scroll(source, body),
        MiddleNetContentKind::GuppyText => parse_guppy(source, body),
        MiddleNetContentKind::MisfinMessage => parse_misfin(source, body),
        MiddleNetContentKind::Html => {
            Err("HTML adaptation is still delegated to the existing web viewers.".to_string())
        }
    }
}

pub fn adapt_streaming(source: &MiddleNetSource, body: &str) -> Result<Vec<DocumentDelta>, String> {
    adapt(source, body).map(|document| vec![DocumentDelta::Replace(document)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::document::SemanticBlock;

    // Per-protocol parser tests live in each per-protocol crate
    // (middlenet-gopher, middlenet-markdown, middlenet-plain-text,
    // middlenet-feed). The tests below only verify that the
    // dispatcher routes each content kind to the correct parser.

    #[test]
    fn adapt_dispatches_gopher_to_semantic_links() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GopherMap);
        let document = adapt(
            &source,
            "iWelcome\tfake\tfake\t70\r\n1Docs\t/docs\texample.com\t70\r\n.\r\n",
        )
        .expect("gopher adapt succeeds");
        assert!(matches!(
            document.blocks.get(1),
            Some(SemanticBlock::Link { text, .. }) if text == "Docs"
        ));
    }

    #[test]
    fn adapt_dispatches_markdown() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown);
        let document = adapt(&source, "# Title\n").expect("markdown adapt succeeds");
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Heading { level: 1, text }) if text == "Title"
        ));
    }

    #[test]
    fn adapt_dispatches_plain_text() {
        let source = MiddleNetSource::new(MiddleNetContentKind::PlainText);
        let document = adapt(&source, "hello\n").expect("plain text adapt succeeds");
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Paragraph(text)) if text == "hello"
        ));
    }

    #[test]
    fn adapt_dispatches_rss_feed() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Rss);
        let document = adapt(
            &source,
            r#"<?xml version="1.0"?>
                <rss version="2.0">
                    <channel><title>x</title></channel>
                </rss>"#,
        )
        .expect("rss adapt succeeds");
        assert_eq!(document.meta.title.as_deref(), Some("x"));
    }

    #[test]
    fn adapt_rejects_html() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Html);
        let result = adapt(&source, "<html></html>");
        assert!(result.is_err());
    }
}
