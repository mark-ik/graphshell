/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use serde::{Deserialize, Serialize};

use crate::source::{MiddleNetContentKind, MiddleNetSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentTrustState {
    Trusted,
    Tofu,
    Insecure,
    Broken,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentDiagnostic {
    UnsupportedConstruct { detail: String },
    DegradedRendering { detail: String },
    ParseWarning { detail: String },
    RawSourceFallback,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDiagnostics {
    pub flags: Vec<DocumentDiagnostic>,
}

impl DocumentDiagnostics {
    pub fn push(&mut self, diagnostic: DocumentDiagnostic) {
        self.flags.push(diagnostic);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkTarget {
    pub href: String,
    pub title: Option<String>,
}

impl LinkTarget {
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            title: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentProvenance {
    pub source_kind: MiddleNetContentKind,
    pub canonical_uri: Option<String>,
    pub fetched_at: Option<String>,
    pub source_label: Option<String>,
}

impl DocumentProvenance {
    pub fn for_source(source: &MiddleNetSource) -> Self {
        Self {
            source_kind: source.content_kind,
            canonical_uri: source.canonical_uri.clone(),
            fetched_at: None,
            source_label: Some(source.content_kind.label().to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub canonical_uri: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content_kind: MiddleNetContentKind,
    pub source_label: Option<String>,
    pub fetched_at: Option<String>,
    pub trust_state: DocumentTrustState,
    pub diagnostics: DocumentDiagnostics,
    pub alternate_open_targets: Vec<LinkTarget>,
    pub raw_source_available: bool,
    pub article_hint: Option<String>,
    pub feed_hint: Option<String>,
}

impl DocumentMeta {
    pub fn for_source(source: &MiddleNetSource) -> Self {
        Self {
            canonical_uri: source.canonical_uri.clone(),
            title: source.title_hint.clone(),
            subtitle: None,
            content_kind: source.content_kind,
            source_label: Some(source.content_kind.label().to_string()),
            fetched_at: None,
            trust_state: DocumentTrustState::Unknown,
            diagnostics: DocumentDiagnostics::default(),
            alternate_open_targets: Vec::new(),
            raw_source_available: false,
            article_hint: None,
            feed_hint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticBlock {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph(String),
    Link {
        text: String,
        target: LinkTarget,
    },
    Quote(String),
    CodeFence {
        lang: Option<String>,
        text: String,
    },
    List {
        ordered: bool,
        items: Vec<String>,
    },
    Rule,
    FeedHeader {
        title: String,
        subtitle: Option<String>,
        summary: Option<String>,
        source_link: Option<LinkTarget>,
    },
    FeedEntry {
        title: String,
        date: Option<String>,
        summary: Option<String>,
        article_link: Option<LinkTarget>,
        source_link: Option<LinkTarget>,
    },
    MetadataRow {
        label: String,
        value: String,
    },
    Badge {
        text: String,
    },
    RawSourceNotice {
        note: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentDelta {
    Replace(SemanticDocument),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticDocument {
    pub meta: DocumentMeta,
    pub provenance: DocumentProvenance,
    pub blocks: Vec<SemanticBlock>,
}

pub type SimpleDocument = SemanticDocument;
pub type SimpleBlock = SemanticBlock;

impl SemanticDocument {
    pub fn new(
        meta: DocumentMeta,
        provenance: DocumentProvenance,
        blocks: Vec<SemanticBlock>,
    ) -> Self {
        Self {
            meta,
            provenance,
            blocks,
        }
    }

    pub fn empty_for_source(source: &MiddleNetSource) -> Self {
        Self::new(
            DocumentMeta::for_source(source),
            DocumentProvenance::for_source(source),
            Vec::new(),
        )
    }

    pub fn from_blocks(source: &MiddleNetSource, blocks: Vec<SemanticBlock>) -> Self {
        let mut document = Self::empty_for_source(source);
        document.blocks = blocks;
        if document.meta.title.is_none() {
            document.meta.title = document.first_title().map(ToOwned::to_owned);
        }
        document
    }

    pub fn first_title(&self) -> Option<&str> {
        self.blocks.iter().find_map(|block| match block {
            SemanticBlock::FeedHeader { title, .. } => Some(title.as_str()),
            SemanticBlock::Heading { level: 1, text } => Some(text.as_str()),
            _ => None,
        })
    }

    pub fn to_html(&self) -> String {
        let mut html = String::new();
        for block in &self.blocks {
            match block {
                SemanticBlock::Heading { level, text } => {
                    html.push_str(&format!("<h{level}>{text}</h{level}>\n"));
                }
                SemanticBlock::Paragraph(text) => {
                    html.push_str(&format!("<p>{text}</p>\n"));
                }
                SemanticBlock::Link { text, target } => {
                    let href = target.href.replace("\"", "&quot;");
                    html.push_str(&format!("<p><a href=\"{href}\">{text}</a></p>\n"));
                }
                SemanticBlock::Quote(text) => {
                    html.push_str(&format!("<blockquote>{text}</blockquote>\n"));
                }
                SemanticBlock::CodeFence { lang, text } => {
                    let class = lang
                        .as_deref()
                        .map(|value| format!(" class=\"language-{value}\""))
                        .unwrap_or_default();
                    html.push_str(&format!("<pre><code{class}>{text}</code></pre>\n"));
                }
                SemanticBlock::List { ordered, items } => {
                    let tag = if *ordered { "ol" } else { "ul" };
                    html.push_str(&format!("<{tag}>\n"));
                    for item in items {
                        html.push_str(&format!("  <li>{item}</li>\n"));
                    }
                    html.push_str(&format!("</{tag}>\n"));
                }
                SemanticBlock::Rule => html.push_str("<hr>\n"),
                SemanticBlock::FeedHeader {
                    title,
                    subtitle,
                    summary,
                    source_link,
                } => {
                    html.push_str(&format!("<header><h1>{title}</h1>"));
                    if let Some(subtitle) = subtitle {
                        html.push_str(&format!("<h2>{subtitle}</h2>"));
                    }
                    if let Some(summary) = summary {
                        html.push_str(&format!("<p>{summary}</p>"));
                    }
                    if let Some(link) = source_link {
                        let href = link.href.replace("\"", "&quot;");
                        let label = link.title.as_deref().unwrap_or("Open source");
                        html.push_str(&format!("<p><a href=\"{href}\">{label}</a></p>"));
                    }
                    html.push_str("</header>\n");
                }
                SemanticBlock::FeedEntry {
                    title,
                    date,
                    summary,
                    article_link,
                    source_link,
                } => {
                    html.push_str("<article>");
                    html.push_str(&format!("<h2>{title}</h2>"));
                    if let Some(date) = date {
                        html.push_str(&format!("<p>{date}</p>"));
                    }
                    if let Some(summary) = summary {
                        html.push_str(&format!("<p>{summary}</p>"));
                    }
                    if let Some(link) = article_link {
                        let href = link.href.replace("\"", "&quot;");
                        let label = link.title.as_deref().unwrap_or("Open article");
                        html.push_str(&format!("<p><a href=\"{href}\">{label}</a></p>"));
                    }
                    if let Some(link) = source_link {
                        let href = link.href.replace("\"", "&quot;");
                        let label = link.title.as_deref().unwrap_or("Open source");
                        html.push_str(&format!("<p><a href=\"{href}\">{label}</a></p>"));
                    }
                    html.push_str("</article>\n");
                }
                SemanticBlock::MetadataRow { label, value } => {
                    html.push_str(&format!("<p><strong>{label}:</strong> {value}</p>\n"));
                }
                SemanticBlock::Badge { text } => {
                    html.push_str(&format!("<p><em>{text}</em></p>\n"));
                }
                SemanticBlock::RawSourceNotice { note } => {
                    html.push_str(&format!("<p>{note}</p>\n"));
                }
            }
        }
        html
    }

    pub fn to_gophermap(&self, hostname: &str, port: u16) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            block.write_gophermap(&mut out, hostname, port);
        }
        out.push_str(".\r\n");
        out
    }

    pub fn to_finger_text(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            block.write_finger_text(&mut out);
        }
        out
    }

    pub fn to_gemini(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            block.write_gemini(&mut out);
        }
        out
    }

    pub fn from_gemini(text: &str) -> Self {
        let source = MiddleNetSource::new(MiddleNetContentKind::GeminiText);
        let mut blocks = Vec::new();
        let mut in_preformat = false;
        let mut preformat_lang: Option<String> = None;
        let mut preformat_lines: Vec<String> = Vec::new();
        let mut list_items: Vec<String> = Vec::new();

        let flush_list = |items: &mut Vec<String>, blocks: &mut Vec<SemanticBlock>| {
            if !items.is_empty() {
                blocks.push(SemanticBlock::List {
                    ordered: false,
                    items: std::mem::take(items),
                });
            }
        };

        for line in text.lines() {
            if in_preformat {
                if line.starts_with("```") {
                    in_preformat = false;
                    blocks.push(SemanticBlock::CodeFence {
                        lang: preformat_lang.take(),
                        text: preformat_lines.join("\n"),
                    });
                    preformat_lines.clear();
                } else {
                    preformat_lines.push(line.to_string());
                }
                continue;
            }

            if line.starts_with("```") {
                flush_list(&mut list_items, &mut blocks);
                let lang_hint = line[3..].trim();
                preformat_lang = if lang_hint.is_empty() {
                    None
                } else {
                    Some(lang_hint.to_string())
                };
                in_preformat = true;
                continue;
            }

            if line.starts_with("### ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SemanticBlock::Heading {
                    level: 3,
                    text: line[4..].to_string(),
                });
            } else if line.starts_with("## ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SemanticBlock::Heading {
                    level: 2,
                    text: line[3..].to_string(),
                });
            } else if line.starts_with("# ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SemanticBlock::Heading {
                    level: 1,
                    text: line[2..].to_string(),
                });
            } else if line.starts_with("=>") {
                flush_list(&mut list_items, &mut blocks);
                let rest = line[2..].trim();
                let (href, label) = if let Some(space_index) =
                    rest.find(|character: char| character.is_ascii_whitespace())
                {
                    let url = rest[..space_index].trim();
                    let text = rest[space_index..].trim();
                    (url, if text.is_empty() { url } else { text })
                } else {
                    (rest, rest)
                };
                blocks.push(SemanticBlock::Link {
                    text: label.to_string(),
                    target: LinkTarget::new(href.to_string()),
                });
            } else if line.starts_with("> ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SemanticBlock::Quote(line[2..].to_string()));
            } else if line.starts_with("* ") {
                list_items.push(line[2..].to_string());
            } else if line.trim().is_empty() {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SemanticBlock::Rule);
            } else {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SemanticBlock::Paragraph(line.to_string()));
            }
        }

        if !list_items.is_empty() {
            blocks.push(SemanticBlock::List {
                ordered: false,
                items: list_items,
            });
        }

        if in_preformat && !preformat_lines.is_empty() {
            blocks.push(SemanticBlock::CodeFence {
                lang: preformat_lang,
                text: preformat_lines.join("\n"),
            });
        }

        let mut document = Self::from_blocks(&source, blocks);
        document.meta.title = document.first_title().map(ToOwned::to_owned);
        document
    }
}

impl SemanticBlock {
    fn write_gemini(&self, out: &mut String) {
        match self {
            SemanticBlock::Heading { level, text } => {
                let prefix = match level {
                    1 => "# ",
                    2 => "## ",
                    _ => "### ",
                };
                out.push_str(prefix);
                out.push_str(text);
                out.push('\n');
            }
            SemanticBlock::Paragraph(text) => {
                out.push_str(text);
                out.push('\n');
            }
            SemanticBlock::Link { text, target } => {
                out.push_str("=> ");
                out.push_str(&target.href);
                if text != &target.href {
                    out.push(' ');
                    out.push_str(text);
                }
                out.push('\n');
            }
            SemanticBlock::Quote(text) => {
                out.push_str("> ");
                out.push_str(text);
                out.push('\n');
            }
            SemanticBlock::CodeFence { lang, text } => {
                out.push_str("```");
                if let Some(language) = lang {
                    out.push_str(language);
                }
                out.push('\n');
                out.push_str(text);
                out.push('\n');
                out.push_str("```\n");
            }
            SemanticBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        out.push_str(&format!("{}. {}\n", index + 1, item));
                    } else {
                        out.push_str("* ");
                        out.push_str(item);
                        out.push('\n');
                    }
                }
            }
            SemanticBlock::Rule => out.push('\n'),
            SemanticBlock::FeedHeader {
                title,
                subtitle,
                summary,
                source_link,
            } => {
                out.push_str("# ");
                out.push_str(title);
                out.push('\n');
                if let Some(subtitle) = subtitle {
                    out.push_str("## ");
                    out.push_str(subtitle);
                    out.push('\n');
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push('\n');
                }
                if let Some(link) = source_link {
                    out.push_str("=> ");
                    out.push_str(&link.href);
                    out.push(' ');
                    out.push_str(link.title.as_deref().unwrap_or("Open source"));
                    out.push('\n');
                }
            }
            SemanticBlock::FeedEntry {
                title,
                date,
                summary,
                article_link,
                source_link,
            } => {
                out.push_str("## ");
                out.push_str(title);
                out.push('\n');
                if let Some(date) = date {
                    out.push_str("> ");
                    out.push_str(date);
                    out.push('\n');
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push('\n');
                }
                if let Some(link) = article_link {
                    out.push_str("=> ");
                    out.push_str(&link.href);
                    out.push(' ');
                    out.push_str(link.title.as_deref().unwrap_or("Open article"));
                    out.push('\n');
                }
                if let Some(link) = source_link {
                    out.push_str("=> ");
                    out.push_str(&link.href);
                    out.push(' ');
                    out.push_str(link.title.as_deref().unwrap_or("Open source"));
                    out.push('\n');
                }
                out.push('\n');
            }
            SemanticBlock::MetadataRow { label, value } => {
                out.push_str(label);
                out.push_str(": ");
                out.push_str(value);
                out.push('\n');
            }
            SemanticBlock::Badge { text } => {
                out.push_str("> ");
                out.push_str(text);
                out.push('\n');
            }
            SemanticBlock::RawSourceNotice { note } => {
                out.push_str(note);
                out.push('\n');
            }
        }
    }

    fn write_gophermap(&self, out: &mut String, hostname: &str, port: u16) {
        let info = |text: &str, out: &mut String| {
            let safe = text.replace('\t', " ");
            out.push_str(&format!("i{safe}\tfake\tfake\t70\r\n"));
        };

        match self {
            SemanticBlock::Heading { level, text } => {
                if *level > 1 {
                    info("", out);
                }
                let underline = "=".repeat(text.len().min(60));
                info(text, out);
                info(&underline, out);
            }
            SemanticBlock::Paragraph(text) => {
                for chunk in wrap_text(text, 70) {
                    info(&chunk, out);
                }
            }
            SemanticBlock::Link { text, target } => {
                let href = &target.href;
                let item_type = if href.starts_with("gopher://") {
                    '1'
                } else {
                    '0'
                };
                let safe_text = text.replace('\t', " ");
                let safe_href = href.replace('\t', " ");
                let selector = if href.starts_with("gemini://")
                    || href.starts_with("http://")
                    || href.starts_with("https://")
                {
                    info(&format!("{safe_text} [{safe_href}]"), out);
                    return;
                } else {
                    safe_href.clone()
                };
                out.push_str(&format!(
                    "{item_type}{safe_text}\t{selector}\t{hostname}\t{port}\r\n"
                ));
            }
            SemanticBlock::Quote(text) => info(&format!("> {text}"), out),
            SemanticBlock::CodeFence { lang, text } => {
                if let Some(language) = lang {
                    info(&format!("[{language}]"), out);
                }
                for line in text.lines() {
                    info(&format!("  {line}"), out);
                }
            }
            SemanticBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        info(&format!("{}. {item}", index + 1), out);
                    } else {
                        info(&format!("* {item}"), out);
                    }
                }
            }
            SemanticBlock::Rule => {
                info("", out);
                info(&"-".repeat(40), out);
                info("", out);
            }
            SemanticBlock::FeedHeader {
                title,
                subtitle,
                summary,
                source_link,
            } => {
                info(title, out);
                if let Some(subtitle) = subtitle {
                    info(subtitle, out);
                }
                if let Some(summary) = summary {
                    for chunk in wrap_text(summary, 70) {
                        info(&chunk, out);
                    }
                }
                if let Some(link) = source_link {
                    SemanticBlock::Link {
                        text: link
                            .title
                            .clone()
                            .unwrap_or_else(|| "Open source".to_string()),
                        target: link.clone(),
                    }
                    .write_gophermap(out, hostname, port);
                }
            }
            SemanticBlock::FeedEntry {
                title,
                date,
                summary,
                article_link,
                source_link,
            } => {
                info(title, out);
                if let Some(date) = date {
                    info(date, out);
                }
                if let Some(summary) = summary {
                    for chunk in wrap_text(summary, 70) {
                        info(&chunk, out);
                    }
                }
                if let Some(link) = article_link {
                    SemanticBlock::Link {
                        text: link
                            .title
                            .clone()
                            .unwrap_or_else(|| "Open article".to_string()),
                        target: link.clone(),
                    }
                    .write_gophermap(out, hostname, port);
                }
                if let Some(link) = source_link {
                    SemanticBlock::Link {
                        text: link
                            .title
                            .clone()
                            .unwrap_or_else(|| "Open source".to_string()),
                        target: link.clone(),
                    }
                    .write_gophermap(out, hostname, port);
                }
            }
            SemanticBlock::MetadataRow { label, value } => info(&format!("{label}: {value}"), out),
            SemanticBlock::Badge { text } => info(text, out),
            SemanticBlock::RawSourceNotice { note } => info(note, out),
        }
    }

    fn write_finger_text(&self, out: &mut String) {
        match self {
            SemanticBlock::Heading { level, text } => {
                let marker = "#".repeat(*level as usize);
                out.push_str(&format!("{marker} {text}\n"));
            }
            SemanticBlock::Paragraph(text) => {
                out.push_str(text);
                out.push('\n');
            }
            SemanticBlock::Link { text, target } => {
                if text == &target.href {
                    out.push_str(&target.href);
                } else {
                    out.push_str(&format!("{text} ({})", target.href));
                }
                out.push('\n');
            }
            SemanticBlock::Quote(text) => out.push_str(&format!("> {text}\n")),
            SemanticBlock::CodeFence { lang, text } => {
                if let Some(language) = lang {
                    out.push_str(&format!("  [{language}]\n"));
                }
                for line in text.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
            SemanticBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        out.push_str(&format!("{}. {item}\n", index + 1));
                    } else {
                        out.push_str(&format!("* {item}\n"));
                    }
                }
            }
            SemanticBlock::Rule => {
                out.push_str(&"-".repeat(40));
                out.push('\n');
            }
            SemanticBlock::FeedHeader {
                title,
                subtitle,
                summary,
                source_link,
            } => {
                out.push_str(&format!("# {title}\n"));
                if let Some(subtitle) = subtitle {
                    out.push_str(&format!("## {subtitle}\n"));
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push('\n');
                }
                if let Some(link) = source_link {
                    let text = link.title.as_deref().unwrap_or("Open source");
                    out.push_str(&format!("{text} ({})\n", link.href));
                }
            }
            SemanticBlock::FeedEntry {
                title,
                date,
                summary,
                article_link,
                source_link,
            } => {
                out.push_str(&format!("## {title}\n"));
                if let Some(date) = date {
                    out.push_str(&format!("> {date}\n"));
                }
                if let Some(summary) = summary {
                    out.push_str(summary);
                    out.push('\n');
                }
                if let Some(link) = article_link {
                    let text = link.title.as_deref().unwrap_or("Open article");
                    out.push_str(&format!("{text} ({})\n", link.href));
                }
                if let Some(link) = source_link {
                    let text = link.title.as_deref().unwrap_or("Open source");
                    out.push_str(&format!("{text} ({})\n", link.href));
                }
            }
            SemanticBlock::MetadataRow { label, value } => {
                out.push_str(&format!("{label}: {value}\n"));
            }
            SemanticBlock::Badge { text } => {
                out.push_str(text);
                out.push('\n');
            }
            SemanticBlock::RawSourceNotice { note } => {
                out.push_str(note);
                out.push('\n');
            }
        }
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.len() <= max_width {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current.clear();
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_gemini_headings_and_links() {
        let input = "# Hello\n## World\n=> gemini://example.com Visit\n";
        let doc = SemanticDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("# Hello\n"));
        assert!(out.contains("## World\n"));
        assert!(out.contains("=> gemini://example.com Visit\n"));
    }

    #[test]
    fn round_trip_list() {
        let input = "* alpha\n* beta\n* gamma\n";
        let doc = SemanticDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("* alpha\n"));
        assert!(out.contains("* beta\n"));
        assert!(out.contains("* gamma\n"));
    }

    #[test]
    fn round_trip_code_fence() {
        let input = "```rust\nlet x = 1;\n```\n";
        let doc = SemanticDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("```rust\n"));
        assert!(out.contains("let x = 1;\n"));
        assert!(out.contains("```\n"));
    }

    #[test]
    fn link_without_label_uses_url_once() {
        let input = "=> gemini://example.com\n";
        let doc = SemanticDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert_eq!(out.matches("gemini://example.com").count(), 1);
    }

    #[test]
    fn quote_round_trip() {
        let input = "> This is a quote\n";
        let doc = SemanticDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("> This is a quote\n"));
    }

    #[test]
    fn gophermap_heading_is_info_line() {
        let doc = SemanticDocument::new(
            DocumentMeta::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            DocumentProvenance::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            vec![SemanticBlock::Heading {
                level: 1,
                text: "My Capsule".to_string(),
            }],
        );
        let map = doc.to_gophermap("localhost", 70);
        assert!(map.contains("iMy Capsule\t"));
        assert!(map.ends_with(".\r\n"));
    }

    #[test]
    fn gophermap_link_becomes_selector() {
        let doc = SemanticDocument::new(
            DocumentMeta::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            DocumentProvenance::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            vec![SemanticBlock::Link {
                text: "My Node".to_string(),
                target: LinkTarget::new("/node/abc"),
            }],
        );
        let map = doc.to_gophermap("myhost", 70);
        assert!(map.contains("0My Node\t/node/abc\tmyhost\t70\r\n"));
    }

    #[test]
    fn finger_text_heading_format() {
        let doc = SemanticDocument::new(
            DocumentMeta::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            DocumentProvenance::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            vec![SemanticBlock::Heading {
                level: 1,
                text: "About Me".to_string(),
            }],
        );
        let text = doc.to_finger_text();
        assert!(text.contains("# About Me\n"));
    }

    #[test]
    fn finger_text_link_with_url() {
        let doc = SemanticDocument::new(
            DocumentMeta::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            DocumentProvenance::for_source(&MiddleNetSource::new(MiddleNetContentKind::GeminiText)),
            vec![SemanticBlock::Link {
                text: "My Site".to_string(),
                target: LinkTarget::new("https://example.com"),
            }],
        );
        let text = doc.to_finger_text();
        assert!(text.contains("My Site (https://example.com)\n"));
    }
}
