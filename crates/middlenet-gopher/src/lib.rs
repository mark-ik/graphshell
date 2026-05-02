/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gophermap (RFC 1436) parser producing a [`SemanticDocument`].
//!
//! This crate is the proof-of-concept for the per-protocol middlenet
//! split: each protocol parser lives in its own crate so third parties
//! (and slim Graphshell builds) can depend on the protocols they
//! actually need without pulling in every other content kind. See
//! `design_docs/.../graphshell_workspace_layout_proposal.md` for the
//! full plan.
//!
//! The parser maps Gopher item types to [`SemanticBlock`] variants:
//!
//! | Item type | Block |
//! |---|---|
//! | `i` | `Paragraph` (or `Rule` if display empty) |
//! | `0` / `1` / `7` / `h` | `Link` (gopher://host:port/selector) |
//! | `3` | `Quote` (error/info) |
//! | other | `Paragraph` (best-effort) |
//!
//! Lines beginning with `.` terminate the document. Empty lines render
//! as `Rule` separators.

use middlenet_core::document::{LinkTarget, SemanticBlock, SemanticDocument};
use middlenet_core::source::MiddleNetSource;

/// Parse a gophermap response body into a [`SemanticDocument`] scoped
/// to `source`. Mirrors the canonical `parse_gophermap` shape used
/// across the rest of the middlenet adapter family.
pub fn parse_gophermap(source: &MiddleNetSource, body: &str) -> SemanticDocument {
    let mut blocks = Vec::new();

    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        if line == "." {
            break;
        }
        if line.is_empty() {
            blocks.push(SemanticBlock::Rule);
            continue;
        }

        let mut characters = line.chars();
        let Some(item_type) = characters.next() else {
            continue;
        };
        let remainder = characters.as_str();
        let mut parts = remainder.split('\t');
        let display = parts.next().unwrap_or_default().trim().to_string();
        let selector = parts.next().unwrap_or_default().trim();
        let host = parts.next().unwrap_or_default().trim();
        let port = parts.next().unwrap_or_default().trim();

        match item_type {
            'i' => {
                if display.is_empty() {
                    blocks.push(SemanticBlock::Rule);
                } else {
                    blocks.push(SemanticBlock::Paragraph(display));
                }
            }
            '0' | '1' | '7' | 'h' => {
                let href = if host.eq_ignore_ascii_case("fake") || selector.is_empty() {
                    selector.to_string()
                } else {
                    let normalized_selector = if selector.starts_with('/') {
                        selector.to_string()
                    } else {
                        format!("/{selector}")
                    };
                    let port_suffix = match port.parse::<u16>() {
                        Ok(70) | Err(_) => String::new(),
                        Ok(value) => format!(":{value}"),
                    };
                    format!("gopher://{host}{port_suffix}{normalized_selector}")
                };
                blocks.push(SemanticBlock::Link {
                    text: display,
                    target: LinkTarget::new(href),
                });
            }
            '3' => blocks.push(SemanticBlock::Quote(display)),
            _ => {
                if !display.is_empty() {
                    blocks.push(SemanticBlock::Paragraph(display));
                }
            }
        }
    }

    SemanticDocument::from_blocks(source, blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn gophermap_links_become_semantic_links() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GopherMap);
        let document = parse_gophermap(
            &source,
            "iWelcome\tfake\tfake\t70\r\n1Docs\t/docs\texample.com\t70\r\n.\r\n",
        );

        assert!(matches!(
            document.blocks.get(1),
            Some(SemanticBlock::Link { text, .. }) if text == "Docs"
        ));
    }

    #[test]
    fn empty_lines_become_rules() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GopherMap);
        let document = parse_gophermap(&source, "iHello\tfake\tfake\t70\r\n\r\niWorld\tfake\tfake\t70\r\n.\r\n");
        assert!(matches!(document.blocks.get(1), Some(SemanticBlock::Rule)));
    }

    #[test]
    fn trailing_dot_terminates_document() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GopherMap);
        let document = parse_gophermap(&source, "iBefore\tfake\tfake\t70\r\n.\r\niAfter\tfake\tfake\t70\r\n");
        // Only "Before" should be present; "After" is past the terminator.
        assert_eq!(document.blocks.len(), 1);
    }

    #[test]
    fn standard_port_70_is_omitted_from_url() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GopherMap);
        let document = parse_gophermap(&source, "0Page\t/p\texample.com\t70\r\n.\r\n");
        if let Some(SemanticBlock::Link { target, .. }) = document.blocks.first() {
            assert_eq!(target.href, "gopher://example.com/p");
        } else {
            panic!("expected a link block");
        }
    }

    #[test]
    fn nonstandard_port_appears_in_url() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GopherMap);
        let document = parse_gophermap(&source, "0Page\t/p\texample.com\t7070\r\n.\r\n");
        if let Some(SemanticBlock::Link { target, .. }) = document.blocks.first() {
            assert_eq!(target.href, "gopher://example.com:7070/p");
        } else {
            panic!("expected a link block");
        }
    }
}
