/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Plain-text adapter — produces a [`SemanticDocument`] where each
//! non-empty input line becomes a `SemanticBlock::Paragraph` and
//! each blank line becomes a `SemanticBlock::Rule`. Used as the
//! parser for both `MiddleNetContentKind::PlainText` and
//! `MiddleNetContentKind::FingerText` (Finger output is line-shaped
//! plain text by protocol).
//!
//! Per-protocol crate per the Slice 49b template; extracted from
//! `middlenet-adapters` in Slice 60.

use middlenet_core::document::{SemanticBlock, SemanticDocument};
use middlenet_core::source::MiddleNetSource;

pub fn parse_plain_text(source: &MiddleNetSource, body: &str) -> SemanticDocument {
    let mut blocks = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blocks.push(SemanticBlock::Rule);
        } else {
            blocks.push(SemanticBlock::Paragraph(trimmed.to_string()));
        }
    }
    SemanticDocument::from_blocks(source, blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn each_line_becomes_a_paragraph() {
        let source = MiddleNetSource::new(MiddleNetContentKind::PlainText);
        let document = parse_plain_text(&source, "first\nsecond\n");
        assert_eq!(document.blocks.len(), 2);
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Paragraph(text)) if text == "first"
        ));
    }

    #[test]
    fn blank_lines_become_rules() {
        let source = MiddleNetSource::new(MiddleNetContentKind::PlainText);
        let document = parse_plain_text(&source, "first\n\nsecond\n");
        assert!(matches!(document.blocks.get(1), Some(SemanticBlock::Rule)));
    }

    #[test]
    fn finger_text_uses_same_parser() {
        // Finger uses the same content shape — line-by-line text.
        let source = MiddleNetSource::new(MiddleNetContentKind::FingerText);
        let document = parse_plain_text(&source, "uptime: 1d\n");
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Paragraph(text)) if text == "uptime: 1d"
        ));
    }
}
