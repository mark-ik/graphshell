/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Syndicated-content feed adapter — RSS 2.0, Atom 1.0, JSON Feed 1.x.
//! All three produce a [`SemanticDocument`] composed of one
//! `FeedHeader` + N `FeedEntry` blocks (with `Rule` separators) and
//! a metadata-enriched `SemanticDocument::meta` (title / subtitle /
//! feed_hint / alternate_open_targets).
//!
//! Per-protocol crate per the Slice 49b template; extracted from
//! `middlenet-adapters` in Slice 60. The three feed flavours share
//! the dispatcher, header/entry append helpers, and text
//! normalisation — keeping them in one crate avoids re-implementing
//! that surface for each format.

mod helpers;

use middlenet_core::document::{LinkTarget, SemanticBlock, SemanticDocument};
use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};
use serde::Deserialize;

use helpers::{
    append_feed_entry, append_feed_header, atom_link_href, child_text, normalize_feed_text,
    normalized_optional_feed_text,
};

/// Top-level feed-parser dispatcher. Routes to the RSS / Atom / JSON
/// Feed parser based on the source's content kind. Returns `Err` for
/// non-feed content kinds.
pub fn parse_feed(source: &MiddleNetSource, body: &str) -> Result<SemanticDocument, String> {
    match source.content_kind {
        MiddleNetContentKind::Rss | MiddleNetContentKind::Atom => {
            let xml = roxmltree::Document::parse(body)
                .map_err(|error| format!("Feed XML parse failed: {error}"))?;
            match source.content_kind {
                MiddleNetContentKind::Rss => parse_rss_feed(source, &xml),
                MiddleNetContentKind::Atom => parse_atom_feed(source, &xml),
                _ => unreachable!("xml feed kinds are matched above"),
            }
        }
        MiddleNetContentKind::JsonFeed => parse_json_feed(source, body),
        _ => Err(format!(
            "Feed adapter does not support '{}' content.",
            source.content_kind.label()
        )),
    }
}

#[derive(Debug, Deserialize)]
struct JsonFeed {
    title: Option<String>,
    home_page_url: Option<String>,
    feed_url: Option<String>,
    description: Option<String>,
    #[serde(default)]
    items: Vec<JsonFeedItem>,
}

#[derive(Debug, Deserialize)]
struct JsonFeedItem {
    id: Option<String>,
    url: Option<String>,
    external_url: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    content_text: Option<String>,
    content_html: Option<String>,
    date_published: Option<String>,
    date_modified: Option<String>,
}

fn parse_rss_feed(
    source: &MiddleNetSource,
    xml: &roxmltree::Document<'_>,
) -> Result<SemanticDocument, String> {
    let channel = xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or_else(|| "RSS feed is missing a <channel> element.".to_string())?;

    let title = child_text(channel, "title");
    let description = child_text(channel, "description");
    let link = child_text(channel, "link");

    let mut blocks = Vec::new();
    append_feed_header(
        &mut blocks,
        title.as_deref(),
        None,
        description.as_deref(),
        link.as_deref(),
    );

    let mut entry_count = 0;
    for item in channel
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
    {
        let entry_title = child_text(item, "title").or_else(|| child_text(item, "guid"));
        let entry_date = child_text(item, "pubDate");
        let entry_summary =
            child_text(item, "description").or_else(|| child_text(item, "encoded"));
        let entry_link = child_text(item, "link");
        append_feed_entry(
            &mut blocks,
            entry_title.as_deref(),
            entry_date.as_deref(),
            entry_summary.as_deref(),
            entry_link.as_deref(),
            None,
        );
        entry_count += 1;
    }

    if entry_count == 0 {
        blocks.push(SemanticBlock::Paragraph(
            "This feed does not contain any entries yet.".to_string(),
        ));
    }

    Ok(finalize_feed_document(
        source,
        blocks,
        title,
        description,
        link,
    ))
}

fn parse_atom_feed(
    source: &MiddleNetSource,
    xml: &roxmltree::Document<'_>,
) -> Result<SemanticDocument, String> {
    let feed = xml.root_element();
    if feed.tag_name().name() != "feed" {
        return Err("Atom feed is missing a <feed> root element.".to_string());
    }

    let title = child_text(feed, "title");
    let subtitle = child_text(feed, "subtitle");
    let link = atom_link_href(feed);

    let mut blocks = Vec::new();
    append_feed_header(
        &mut blocks,
        title.as_deref(),
        subtitle.as_deref(),
        None,
        link.as_deref(),
    );

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
            None,
        );
        entry_count += 1;
    }

    if entry_count == 0 {
        blocks.push(SemanticBlock::Paragraph(
            "This feed does not contain any entries yet.".to_string(),
        ));
    }

    Ok(finalize_feed_document(
        source, blocks, title, subtitle, link,
    ))
}

fn parse_json_feed(source: &MiddleNetSource, body: &str) -> Result<SemanticDocument, String> {
    let feed: JsonFeed = serde_json::from_str(body)
        .map_err(|error| format!("JSON Feed parse failed: {error}"))?;

    let title = normalized_optional_feed_text(feed.title.as_deref());
    let description = normalized_optional_feed_text(feed.description.as_deref());
    let link = normalized_optional_feed_text(
        feed.home_page_url.as_deref().or(feed.feed_url.as_deref()),
    );

    let mut blocks = Vec::new();
    append_feed_header(
        &mut blocks,
        title.as_deref(),
        None,
        description.as_deref(),
        link.as_deref(),
    );

    let mut entry_count = 0;
    for item in &feed.items {
        let entry_title = item
            .title
            .as_deref()
            .or(item.id.as_deref())
            .and_then(|value| normalized_optional_feed_text(Some(value)));
        let entry_date = item
            .date_published
            .as_deref()
            .or(item.date_modified.as_deref())
            .and_then(|value| normalized_optional_feed_text(Some(value)));
        let entry_summary = item
            .summary
            .as_deref()
            .or(item.content_text.as_deref())
            .or(item.content_html.as_deref())
            .and_then(|value| normalized_optional_feed_text(Some(value)));
        let entry_link = item
            .url
            .as_deref()
            .or(item.external_url.as_deref())
            .and_then(|value| normalized_optional_feed_text(Some(value)));

        append_feed_entry(
            &mut blocks,
            entry_title.as_deref(),
            entry_date.as_deref(),
            entry_summary.as_deref(),
            entry_link.as_deref(),
            None,
        );
        entry_count += 1;
    }

    if entry_count == 0 {
        blocks.push(SemanticBlock::Paragraph(
            "This feed does not contain any entries yet.".to_string(),
        ));
    }

    Ok(finalize_feed_document(
        source,
        blocks,
        title,
        description,
        link,
    ))
}

fn finalize_feed_document(
    source: &MiddleNetSource,
    blocks: Vec<SemanticBlock>,
    title: Option<String>,
    subtitle: Option<String>,
    link: Option<String>,
) -> SemanticDocument {
    let mut document = SemanticDocument::from_blocks(source, blocks);
    document.meta.title = title.or_else(|| source.title_hint.clone());
    document.meta.subtitle = subtitle;
    document.meta.feed_hint = document.meta.title.clone();
    document.meta.raw_source_available = true;
    if let Some(link) = link {
        document
            .meta
            .alternate_open_targets
            .push(LinkTarget::new(link).with_title("Open feed source"));
    }
    document
}

// Suppress unused-import warning when normalize_feed_text isn't referenced
// directly in this file (it's used by helpers; expose at crate-root for
// completeness so external callers can reach it if needed).
#[allow(dead_code)]
fn _normalize_feed_text_passthrough(text: &str) -> String {
    normalize_feed_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_adapter_parses_feed_title_and_entries() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Rss);
        let document = parse_feed(
            &source,
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

        assert_eq!(document.meta.title.as_deref(), Some("Example Feed"));
        assert!(document.blocks.iter().any(
            |block| matches!(block, SemanticBlock::FeedEntry { title, .. } if title == "First item")
        ));
    }

    #[test]
    fn atom_adapter_parses_feed_title_and_entries() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Atom);
        let document = parse_feed(
            &source,
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

        assert_eq!(document.meta.title.as_deref(), Some("Atom Example"));
        assert!(document
            .blocks
            .iter()
            .any(|block| matches!(block, SemanticBlock::FeedEntry { date: Some(_), .. })));
    }

    #[test]
    fn json_feed_adapter_parses_feed_title_and_entries() {
        let source = MiddleNetSource::new(MiddleNetContentKind::JsonFeed);
        let document = parse_feed(
            &source,
            r#"{
                "version": "https://jsonfeed.org/version/1.1",
                "title": "Graphshell Notes",
                "home_page_url": "https://example.com/",
                "description": "Recent updates from <b>Graphshell</b>",
                "items": [
                    {
                        "id": "entry-1",
                        "url": "https://example.com/posts/1",
                        "title": "First note",
                        "content_html": "Hello <i>world</i>",
                        "date_published": "2026-04-08T10:00:00Z"
                    }
                ]
            }"#,
        )
        .expect("json feed should parse");

        assert_eq!(document.meta.title.as_deref(), Some("Graphshell Notes"));
        assert!(document.blocks.iter().any(|block| matches!(
            block,
            SemanticBlock::FeedEntry { summary: Some(summary), .. } if summary == "Hello world"
        )));
    }

    #[test]
    fn rejects_non_feed_content_kind() {
        let source = MiddleNetSource::new(MiddleNetContentKind::Markdown);
        let result = parse_feed(&source, "anything");
        assert!(result.is_err());
    }
}
