/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::middlenet::document::{SimpleBlock, SimpleDocument};
use crate::middlenet::source::MiddleNetContentKind;

pub(crate) fn parse_gophermap(body: &str) -> SimpleDocument {
    let mut blocks = Vec::new();

    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        if line == "." {
            break;
        }
        if line.is_empty() {
            blocks.push(SimpleBlock::Rule);
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
                    blocks.push(SimpleBlock::Rule);
                } else {
                    blocks.push(SimpleBlock::Paragraph(display));
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
                blocks.push(SimpleBlock::Link {
                    text: display,
                    href,
                });
            }
            '3' => blocks.push(SimpleBlock::Quote(display)),
            _ => {
                if !display.is_empty() {
                    blocks.push(SimpleBlock::Paragraph(display));
                }
            }
        }
    }

    SimpleDocument::Blocks(blocks)
}

pub(crate) fn parse_markdown(body: &str) -> SimpleDocument {
    let mut blocks = Vec::new();
    let mut unordered_items = Vec::new();
    let mut ordered_items = Vec::new();
    let mut in_code_fence = false;
    let mut code_language = None;
    let mut code_lines = Vec::new();

    let flush_lists = |unordered_items: &mut Vec<String>,
                       ordered_items: &mut Vec<String>,
                       blocks: &mut Vec<SimpleBlock>| {
        if !unordered_items.is_empty() {
            blocks.push(SimpleBlock::List {
                ordered: false,
                items: std::mem::take(unordered_items),
            });
        }
        if !ordered_items.is_empty() {
            blocks.push(SimpleBlock::List {
                ordered: true,
                items: std::mem::take(ordered_items),
            });
        }
    };

    for line in body.lines() {
        let trimmed = line.trim_end();

        if in_code_fence {
            if trimmed.starts_with("```") {
                blocks.push(SimpleBlock::CodeFence {
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
            blocks.push(SimpleBlock::Rule);
            continue;
        }

        if let Some((hashes, rest)) = trimmed.split_once(' ')
            && hashes.chars().all(|character| character == '#')
            && !hashes.is_empty()
            && hashes.len() <= 6
        {
            flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);
            blocks.push(SimpleBlock::Heading {
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
            blocks.push(SimpleBlock::Quote(quote.trim().to_string()));
            continue;
        }

        if trimmed == "---" || trimmed == "***" {
            blocks.push(SimpleBlock::Rule);
            continue;
        }

        if let Some((text, href)) = parse_markdown_link_line(trimmed) {
            blocks.push(SimpleBlock::Link { text, href });
            continue;
        }

        blocks.push(SimpleBlock::Paragraph(trimmed.to_string()));
    }

    flush_lists(&mut unordered_items, &mut ordered_items, &mut blocks);

    if in_code_fence {
        blocks.push(SimpleBlock::CodeFence {
            lang: code_language,
            text: code_lines.join("\n"),
        });
    }

    SimpleDocument::Blocks(blocks)
}

pub(crate) fn parse_plain_text(body: &str) -> SimpleDocument {
    let mut blocks = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blocks.push(SimpleBlock::Rule);
        } else {
            blocks.push(SimpleBlock::Paragraph(trimmed.to_string()));
        }
    }
    SimpleDocument::Blocks(blocks)
}

pub(crate) fn parse_feed(
    content_kind: MiddleNetContentKind,
    body: &str,
) -> Result<(SimpleDocument, Option<String>), String> {
    let xml = roxmltree::Document::parse(body)
        .map_err(|error| format!("Feed XML parse failed: {error}"))?;

    match content_kind {
        MiddleNetContentKind::Rss => parse_rss_feed(&xml),
        MiddleNetContentKind::Atom => parse_atom_feed(&xml),
        _ => Err(format!(
            "Feed adapter does not support '{}' content.",
            content_kind.label()
        )),
    }
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

fn parse_rss_feed(
    xml: &roxmltree::Document<'_>,
) -> Result<(SimpleDocument, Option<String>), String> {
    let channel = xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or_else(|| "RSS feed is missing a <channel> element.".to_string())?;

    let title = child_text(channel, "title");
    let description = child_text(channel, "description");
    let link = child_text(channel, "link");

    let mut blocks = Vec::new();
    append_feed_header(&mut blocks, title.as_deref(), description.as_deref(), link.as_deref());

    let mut entry_count = 0;
    for item in channel
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
    {
        let entry_title = child_text(item, "title").or_else(|| child_text(item, "guid"));
        let entry_date = child_text(item, "pubDate");
        let entry_summary = child_text(item, "description").or_else(|| child_text(item, "encoded"));
        let entry_link = child_text(item, "link");
        append_feed_entry(
            &mut blocks,
            entry_title.as_deref(),
            entry_date.as_deref(),
            entry_summary.as_deref(),
            entry_link.as_deref(),
        );
        entry_count += 1;
    }

    if entry_count == 0 {
        blocks.push(SimpleBlock::Paragraph(
            "This feed does not contain any entries yet.".to_string(),
        ));
    }

    trim_trailing_rule(&mut blocks);
    Ok((SimpleDocument::Blocks(blocks), title))
}

fn parse_atom_feed(
    xml: &roxmltree::Document<'_>,
) -> Result<(SimpleDocument, Option<String>), String> {
    let feed = xml.root_element();
    if feed.tag_name().name() != "feed" {
        return Err("Atom feed is missing a <feed> root element.".to_string());
    }

    let title = child_text(feed, "title");
    let subtitle = child_text(feed, "subtitle");
    let link = atom_link_href(feed);

    let mut blocks = Vec::new();
    append_feed_header(&mut blocks, title.as_deref(), subtitle.as_deref(), link.as_deref());

    let mut entry_count = 0;
    for entry in feed
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "entry")
    {
        let entry_title = child_text(entry, "title");
        let entry_date = child_text(entry, "updated").or_else(|| child_text(entry, "published"));
        let entry_summary = child_text(entry, "summary").or_else(|| child_text(entry, "content"));
        let entry_link = atom_link_href(entry);
        append_feed_entry(
            &mut blocks,
            entry_title.as_deref(),
            entry_date.as_deref(),
            entry_summary.as_deref(),
            entry_link.as_deref(),
        );
        entry_count += 1;
    }

    if entry_count == 0 {
        blocks.push(SimpleBlock::Paragraph(
            "This feed does not contain any entries yet.".to_string(),
        ));
    }

    trim_trailing_rule(&mut blocks);
    Ok((SimpleDocument::Blocks(blocks), title))
}

fn append_feed_header(
    blocks: &mut Vec<SimpleBlock>,
    title: Option<&str>,
    summary: Option<&str>,
    link: Option<&str>,
) {
    blocks.push(SimpleBlock::Heading {
        level: 1,
        text: title.unwrap_or("Feed").trim().to_string(),
    });

    if let Some(summary) = summary {
        let normalized = normalize_feed_text(summary);
        if !normalized.is_empty() {
            blocks.push(SimpleBlock::Paragraph(normalized));
        }
    }

    if let Some(link) = link
        && !link.trim().is_empty()
    {
        blocks.push(SimpleBlock::Link {
            text: "Open feed source".to_string(),
            href: link.trim().to_string(),
        });
    }

    blocks.push(SimpleBlock::Rule);
}

fn append_feed_entry(
    blocks: &mut Vec<SimpleBlock>,
    title: Option<&str>,
    date: Option<&str>,
    summary: Option<&str>,
    link: Option<&str>,
) {
    blocks.push(SimpleBlock::Heading {
        level: 2,
        text: title.unwrap_or("Untitled entry").trim().to_string(),
    });

    if let Some(date) = date {
        let normalized = normalize_feed_text(date);
        if !normalized.is_empty() {
            blocks.push(SimpleBlock::Quote(normalized));
        }
    }

    if let Some(summary) = summary {
        let normalized = normalize_feed_text(summary);
        if !normalized.is_empty() {
            blocks.push(SimpleBlock::Paragraph(normalized));
        }
    }

    if let Some(link) = link
        && !link.trim().is_empty()
    {
        blocks.push(SimpleBlock::Link {
            text: "Open article".to_string(),
            href: link.trim().to_string(),
        });
    }

    blocks.push(SimpleBlock::Rule);
}

fn child_text(node: roxmltree::Node<'_, '_>, local_name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == local_name)
        .and_then(|child| child.text())
        .map(normalize_feed_text)
        .filter(|text| !text.is_empty())
}

fn atom_link_href(node: roxmltree::Node<'_, '_>) -> Option<String> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == "link")
        .find_map(|link| {
            let rel = link.attribute("rel").unwrap_or("alternate");
            (rel == "alternate" || rel == "self")
                .then(|| link.attribute("href"))
                .flatten()
        })
        .map(str::trim)
        .filter(|href| !href.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_feed_text(text: &str) -> String {
    let without_tags = strip_markup(text);
    without_tags
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn strip_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside_tag = false;

    for character in text.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => out.push(character),
            _ => {}
        }
    }

    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn trim_trailing_rule(blocks: &mut Vec<SimpleBlock>) {
    if matches!(blocks.last(), Some(SimpleBlock::Rule)) {
        blocks.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gophermap_links_become_document_links() {
        let document = parse_gophermap(
            "iWelcome\tfake\tfake\t70\r\n1Docs\t/docs\texample.com\t70\r\n.\r\n",
        );

        let SimpleDocument::Blocks(blocks) = document;
        assert!(matches!(blocks.first(), Some(SimpleBlock::Paragraph(_))));
        assert!(matches!(blocks.get(1), Some(SimpleBlock::Link { .. })));
    }

    #[test]
    fn markdown_adapter_recognizes_headings_lists_and_links() {
        let document = parse_markdown("# Title\n- first\n- second\n[Next](gemini://example.com/next)\n");

        let SimpleDocument::Blocks(blocks) = document;
        assert!(matches!(blocks.first(), Some(SimpleBlock::Heading { .. })));
        assert!(matches!(blocks.get(1), Some(SimpleBlock::List { .. })));
        assert!(matches!(blocks.get(2), Some(SimpleBlock::Link { .. })));
    }

        #[test]
        fn rss_adapter_parses_feed_title_and_entries() {
                let (document, title) = parse_feed(
                        MiddleNetContentKind::Rss,
                        r#"<?xml version="1.0"?>
                        <rss version="2.0">
                            <channel>
                                <title>Example Feed</title>
                                <description>Updates from Graphshell</description>
                                <link>https://example.com/feed</link>
                                <item>
                                    <title>First item</title>
                                    <link>https://example.com/first</link>
                                    <description>Hello <b>world</b></description>
                                    <pubDate>Tue, 08 Apr 2026 10:00:00 GMT</pubDate>
                                </item>
                            </channel>
                        </rss>"#,
                )
                .expect("rss should parse");

                assert_eq!(title.as_deref(), Some("Example Feed"));
                let SimpleDocument::Blocks(blocks) = document;
                assert!(matches!(blocks.first(), Some(SimpleBlock::Heading { .. })));
                assert!(blocks.iter().any(|block| matches!(block, SimpleBlock::Link { href, .. } if href == "https://example.com/first")));
        }

        #[test]
        fn atom_adapter_parses_feed_title_and_entries() {
                let (document, title) = parse_feed(
                        MiddleNetContentKind::Atom,
                        r#"<?xml version="1.0" encoding="utf-8"?>
                        <feed xmlns="http://www.w3.org/2005/Atom">
                            <title>Atom Example</title>
                            <subtitle>Recent notes</subtitle>
                            <link href="https://example.com/atom.xml" rel="self" />
                            <entry>
                                <title>Entry one</title>
                                <updated>2026-04-08T10:00:00Z</updated>
                                <summary>Structured <i>content</i></summary>
                                <link href="https://example.com/entry-one" />
                            </entry>
                        </feed>"#,
                )
                .expect("atom should parse");

                assert_eq!(title.as_deref(), Some("Atom Example"));
                let SimpleDocument::Blocks(blocks) = document;
                assert!(blocks.iter().any(|block| matches!(block, SimpleBlock::Quote(text) if text == "2026-04-08T10:00:00Z")));
                assert!(blocks.iter().any(|block| matches!(block, SimpleBlock::Link { href, .. } if href == "https://example.com/entry-one")));
        }
}