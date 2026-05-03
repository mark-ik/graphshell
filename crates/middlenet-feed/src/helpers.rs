/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Header / entry append helpers + text normalisation shared across
//! the RSS / Atom / JSON Feed parsers. Kept private to the
//! `middlenet-feed` crate because the helpers are tightly coupled to
//! the feed-specific block layout (FeedHeader / FeedEntry / Rule).

use middlenet_core::document::{LinkTarget, SemanticBlock};

pub(super) fn append_feed_header(
    blocks: &mut Vec<SemanticBlock>,
    title: Option<&str>,
    subtitle: Option<&str>,
    summary: Option<&str>,
    link: Option<&str>,
) {
    blocks.push(SemanticBlock::FeedHeader {
        title: title.unwrap_or("Feed").trim().to_string(),
        subtitle: subtitle
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        summary: summary
            .map(normalize_feed_text)
            .filter(|value| !value.is_empty()),
        source_link: link
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|href| LinkTarget::new(href.to_string()).with_title("Open feed source")),
    });
    blocks.push(SemanticBlock::Rule);
}

pub(super) fn append_feed_entry(
    blocks: &mut Vec<SemanticBlock>,
    title: Option<&str>,
    date: Option<&str>,
    summary: Option<&str>,
    article_link: Option<&str>,
    source_link: Option<&str>,
) {
    blocks.push(SemanticBlock::FeedEntry {
        title: title.unwrap_or("Untitled entry").trim().to_string(),
        date: date
            .map(normalize_feed_text)
            .filter(|value| !value.is_empty()),
        summary: summary
            .map(normalize_feed_text)
            .filter(|value| !value.is_empty()),
        article_link: article_link
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|href| LinkTarget::new(href.to_string()).with_title("Open article")),
        source_link: source_link
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|href| LinkTarget::new(href.to_string()).with_title("Open source")),
    });
    blocks.push(SemanticBlock::Rule);
}

pub(super) fn child_text(node: roxmltree::Node<'_, '_>, local_name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == local_name)
        .and_then(|child| child.text())
        .map(normalize_feed_text)
        .filter(|text| !text.is_empty())
}

pub(super) fn atom_link_href(node: roxmltree::Node<'_, '_>) -> Option<String> {
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

pub(super) fn normalize_feed_text(text: &str) -> String {
    let without_tags = strip_markup(text);
    without_tags
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub(super) fn normalized_optional_feed_text(text: Option<&str>) -> Option<String> {
    text.map(normalize_feed_text)
        .filter(|text| !text.is_empty())
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

    out
}
