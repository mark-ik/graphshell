/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gemini (`gemini://`) protocol parser + serializer for gemtext.
//!
//! Slice 61 wrapper: the parsing logic itself currently lives in
//! `middlenet-core`'s `SemanticDocument::from_gemini` /
//! `SemanticDocument::to_gemini` inherent methods (it predates the
//! per-protocol crate split). This crate exposes those as free
//! functions for parity with the rest of the middlenet protocol
//! family. A follow-up slice can move the bodies into this crate and
//! reduce the core methods to thin wrappers — that's the symmetric
//! shape with `middlenet-gopher` etc.
//!
//! The serializer emits gemtext per the [Project Gemini
//! specification](https://geminiprotocol.net/docs/specification.gmi):
//! `# / ## / ###` headings, `=> url label` link lines,
//! ```` ``` ```` preformatted blocks, `> ` quotes, `* ` list items.
//! Feed-shaped blocks (`FeedHeader` / `FeedEntry`) emit as `# title`
//! + `## entry` sections so a feed renders as a readable gemtext page.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

/// Parse a gemtext body into a [`SemanticDocument`]. The `source`
/// argument is currently informational — the parser doesn't read it
/// — but the parameter is preserved for parity with the rest of the
/// per-protocol family and to enable source-aware enrichment in a
/// future slice (e.g., resolving relative `=>` links against the
/// `canonical_uri`).
pub fn parse_gemini(_source: &MiddleNetSource, body: &str) -> SemanticDocument {
    SemanticDocument::from_gemini(body)
}

/// Serialize a [`SemanticDocument`] to gemtext. Round-trip-stable
/// for the gemtext-native subset (headings / paragraphs / links /
/// quotes / lists / code / rules) — feed-shaped blocks render as
/// nested gemini sections rather than round-tripping back to feed
/// XML/JSON.
pub fn serialize_gemini(document: &SemanticDocument) -> String {
    document.to_gemini()
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::document::SemanticBlock;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn parses_headings_links_lists() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText);
        let doc = parse_gemini(
            &source,
            "# Hello\n=> gemini://example.com Visit\n* one\n* two\n",
        );
        assert!(matches!(
            doc.blocks.first(),
            Some(SemanticBlock::Heading { level: 1, text }) if text == "Hello"
        ));
    }

    #[test]
    fn round_trips_basic_gemtext() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText);
        let original = "# Title\n=> gemini://example.com Visit\n";
        let doc = parse_gemini(&source, original);
        let serialized = serialize_gemini(&doc);
        assert!(serialized.contains("# Title\n"));
        assert!(serialized.contains("=> gemini://example.com Visit\n"));
    }

    #[test]
    fn parses_preformatted_blocks() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText);
        let doc = parse_gemini(&source, "```rust\nfn main() {}\n```\n");
        assert!(matches!(
            doc.blocks.first(),
            Some(SemanticBlock::CodeFence { lang: Some(lang), .. })
                if lang == "rust"
        ));
    }
}
