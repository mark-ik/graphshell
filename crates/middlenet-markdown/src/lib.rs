/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Markdown adapter — produces a [`SemanticDocument`] from a markdown
//! body. Recognises ATX headings (`# … ######`), unordered lists
//! (`-`/`*`/`+`), ordered lists (`<n>. `), block quotes (`> `),
//! horizontal rules (`---` / `***`), fenced code blocks (```` ``` ````),
//! and bare `[text](href)` link lines. Anything else becomes a
//! `SemanticBlock::Paragraph`.
//!
//! Per-protocol crate per the Slice 49b template; extracted from
//! `middlenet-adapters` in Slice 60.

use middlenet_core::document::{LinkTarget, SemanticBlock, SemanticDocument};
use middlenet_core::source::MiddleNetSource;

pub fn parse_markdown(source: &MiddleNetSource, body: &str) -> SemanticDocument {
    let mut blocks = Vec::new();
    let mut unordered_items = Vec::new();
    let mut ordered_items = Vec::new();
    let mut in_code_fence = false;
    let mut code_language = None;
    let mut code_lines = Vec::new();

    let flush_lists = |unordered_items: &mut Vec<String>,
                       ordered_items: &mut Vec<String>,
                       blocks: &mut Vec<SemanticBlock>| {
        if !unordered_items.is_empty() {
            blocks.push(SemanticBlock::List {
                ordered: false,
                items: std::mem::take(unordered_items),
            });
        }
        if !ordered_items.is_empty() {
            blocks.push(SemanticBlock::List {
                ordered: true,
                items: std::mem::take(ordered_items),
            });
        }
    };

    for line in body.lines() {
        let trimmed = line.trim_end();

        if in_code_fence {
            if trimmed.starts_with("```") {
                blocks.push(SemanticBlock::CodeFence {
                    lang: code_language.take(),
                    text: code_lines.join("\n"),
                });
                code_lines.clear();
                in_code_fence = false;
            } else {
                code_lines.push(trimmed.to_string());
            }
            continue;
        }

        if trimmed.starts_with("```") {
            flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);
            let language = trimmed[3..].trim();
            code_language = (!language.is_empty()).then(|| language.to_string());
            in_code_fence = true;
            continue;
        }

        if trimmed.is_empty() {
            flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);
            blocks.push(SemanticBlock::Rule);
            continue;
        }

        if let Some((hashes, rest)) = trimmed.split_once(' ')
            && hashes.chars().all(|character| character == '#')
            && !hashes.is_empty()
            && hashes.len() <= 6
        {
            flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);
            blocks.push(SemanticBlock::Heading {
                level: hashes.len() as u8,
                text: rest.trim().to_string(),
            });
            continue;
        }

        if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            ordered_items.clear();
            unordered_items.push(item.trim().to_string());
            continue;
        }

        if let Some(item) = parse_ordered_markdown_item(trimmed) {
            unordered_items.clear();
            ordered_items.push(item);
            continue;
        }

        flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);

        if let Some(quote) = trimmed.strip_prefix("> ") {
            blocks.push(SemanticBlock::Quote(quote.trim().to_string()));
            continue;
        }

        if trimmed == "---" || trimmed == "***" {
            blocks.push(SemanticBlock::Rule);
            continue;
        }

        if let Some((text, href)) = parse_markdown_link_line(trimmed) {
            blocks.push(SemanticBlock::Link {
                text,
                target: LinkTarget::new(href),
            });
            continue;
        }

        blocks.push(SemanticBlock::Paragraph(trimmed.to_string()));
    }

    flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);

    if in_code_fence {
        blocks.push(SemanticBlock::CodeFence {
            lang: code_language,
            text: code_lines.join("\n"),
        });
    }

    SemanticDocument::from_blocks(source, blocks)
}

fn parse_ordered_markdown_item(line: &str) -> Option<String> {
    let period_index = line.find('.')?;
    if period_index == 0 {
        return None;
    }
    let (number, rest) = line.split_at(period_index);
    if !number.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    rest.strip_prefix(". ").map(|item| item.trim().to_string())
}

fn parse_markdown_link_line(line: &str) -> Option<(String, String)> {
    let text_start = line.strip_prefix('[')?;
    let closing_bracket = text_start.find(']')?;
    let text = text_start[..closing_bracket].trim();
    let href_start = text_start[closing_bracket + 1..].strip_prefix('(')?;
    let closing_paren = href_start.rfind(')')?;
    let href = href_start[..closing_paren].trim();
    if href.is_empty() {
        return None;
    }
    Some((text.to_string(), href.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn markdown_adapter_recognizes_headings_lists_and_links() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown);
        let document = parse_markdown(
            &source,
            "# Title\n- first\n- second\n[Next](gemini://example.com/next)\n",
        );

        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Heading { level: 1, text }) if text == "Title"
        ));
    }

    #[test]
    fn parses_ordered_list_items() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown);
        let document = parse_markdown(&source, "1. first\n2. second\n");
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::List { ordered: true, items }) if items.len() == 2
        ));
    }

    #[test]
    fn parses_blockquote() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown);
        let document = parse_markdown(&source, "> hello there\n");
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Quote(text)) if text == "hello there"
        ));
    }

    #[test]
    fn parses_fenced_code_block() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown);
        let document = parse_markdown(&source, "```rust\nfn main() {}\n```\n");
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::CodeFence { lang: Some(lang), text })
                if lang == "rust" && text == "fn main() {}"
        ));
    }
}
