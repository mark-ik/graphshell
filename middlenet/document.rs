/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Format-agnostic block-structured intermediate model for MiddleNet content.
//!
//! This is the current shared document spine for protocol adapters and capsule
//! serializers. It is intentionally pure Rust so it can move into a future
//! portable crate without dragging along Servo or host-only dependencies.

/// Format-agnostic block-structured intermediate model.
#[derive(Debug, Clone)]
pub enum SimpleDocument {
    Blocks(Vec<SimpleBlock>),
}

/// A single content block within a [`SimpleDocument`].
#[derive(Debug, Clone)]
pub enum SimpleBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    Link { text: String, href: String },
    Quote(String),
    CodeFence { lang: Option<String>, text: String },
    List { ordered: bool, items: Vec<String> },
    Rule,
}

impl SimpleDocument {
    /// Serialize this document to Gopher menu format (RFC 1436).
    pub fn to_gophermap(&self, hostname: &str, port: u16) -> String {
        let SimpleDocument::Blocks(blocks) = self;
        let mut out = String::new();
        for block in blocks {
            block.write_gophermap(&mut out, hostname, port);
        }
        out.push_str(".\r\n");
        out
    }

    /// Serialize this document to plain text suitable for Finger responses.
    pub fn to_finger_text(&self) -> String {
        let SimpleDocument::Blocks(blocks) = self;
        let mut out = String::new();
        for block in blocks {
            block.write_finger_text(&mut out);
        }
        out
    }

    /// Serialize this document to `text/gemini` format.
    pub fn to_gemini(&self) -> String {
        let SimpleDocument::Blocks(blocks) = self;
        let mut out = String::new();
        for block in blocks {
            block.write_gemini(&mut out);
        }
        out
    }

    /// Parse `text/gemini` bytes into a `SimpleDocument`.
    pub fn from_gemini(text: &str) -> Self {
        let mut blocks = Vec::new();
        let mut in_preformat = false;
        let mut preformat_lang: Option<String> = None;
        let mut preformat_lines: Vec<String> = Vec::new();
        let mut list_items: Vec<String> = Vec::new();

        let flush_list = |items: &mut Vec<String>, blocks: &mut Vec<SimpleBlock>| {
            if !items.is_empty() {
                blocks.push(SimpleBlock::List {
                    ordered: false,
                    items: std::mem::take(items),
                });
            }
        };

        for line in text.lines() {
            if in_preformat {
                if line.starts_with("```") {
                    in_preformat = false;
                    blocks.push(SimpleBlock::CodeFence {
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
                blocks.push(SimpleBlock::Heading {
                    level: 3,
                    text: line[4..].to_string(),
                });
            } else if line.starts_with("## ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SimpleBlock::Heading {
                    level: 2,
                    text: line[3..].to_string(),
                });
            } else if line.starts_with("# ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SimpleBlock::Heading {
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
                blocks.push(SimpleBlock::Link {
                    text: label.to_string(),
                    href: href.to_string(),
                });
            } else if line.starts_with("> ") {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SimpleBlock::Quote(line[2..].to_string()));
            } else if line.starts_with("* ") {
                list_items.push(line[2..].to_string());
            } else if line.trim().is_empty() {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SimpleBlock::Rule);
            } else {
                flush_list(&mut list_items, &mut blocks);
                blocks.push(SimpleBlock::Paragraph(line.to_string()));
            }
        }

        if !list_items.is_empty() {
            blocks.push(SimpleBlock::List {
                ordered: false,
                items: list_items,
            });
        }

        if in_preformat && !preformat_lines.is_empty() {
            blocks.push(SimpleBlock::CodeFence {
                lang: preformat_lang,
                text: preformat_lines.join("\n"),
            });
        }

        SimpleDocument::Blocks(blocks)
    }
}

impl SimpleBlock {
    fn write_gemini(&self, out: &mut String) {
        match self {
            SimpleBlock::Heading { level, text } => {
                let prefix = match level {
                    1 => "# ",
                    2 => "## ",
                    _ => "### ",
                };
                out.push_str(prefix);
                out.push_str(text);
                out.push('\n');
            }
            SimpleBlock::Paragraph(text) => {
                out.push_str(text);
                out.push('\n');
            }
            SimpleBlock::Link { text, href } => {
                out.push_str("=> ");
                out.push_str(href);
                if text != href {
                    out.push(' ');
                    out.push_str(text);
                }
                out.push('\n');
            }
            SimpleBlock::Quote(text) => {
                out.push_str("> ");
                out.push_str(text);
                out.push('\n');
            }
            SimpleBlock::CodeFence { lang, text } => {
                out.push_str("```");
                if let Some(language) = lang {
                    out.push_str(language);
                }
                out.push('\n');
                out.push_str(text);
                out.push('\n');
                out.push_str("```\n");
            }
            SimpleBlock::List { ordered, items } => {
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
            SimpleBlock::Rule => {
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
            SimpleBlock::Heading { level, text } => {
                if *level > 1 {
                    info("", out);
                }
                let underline = "=".repeat(text.len().min(60));
                info(text, out);
                info(&underline, out);
            }
            SimpleBlock::Paragraph(text) => {
                for chunk in wrap_text(text, 70) {
                    info(&chunk, out);
                }
            }
            SimpleBlock::Link { text, href } => {
                let item_type = if href.starts_with("gopher://") { '1' } else { '0' };
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
            SimpleBlock::Quote(text) => {
                info(&format!("> {text}"), out);
            }
            SimpleBlock::CodeFence { lang, text } => {
                if let Some(language) = lang {
                    info(&format!("[{language}]"), out);
                }
                for line in text.lines() {
                    info(&format!("  {line}"), out);
                }
            }
            SimpleBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        info(&format!("{}. {item}", index + 1), out);
                    } else {
                        info(&format!("* {item}"), out);
                    }
                }
            }
            SimpleBlock::Rule => {
                info("", out);
                info(&"-".repeat(40), out);
                info("", out);
            }
        }
    }

    fn write_finger_text(&self, out: &mut String) {
        match self {
            SimpleBlock::Heading { level, text } => {
                let marker = "#".repeat(*level as usize);
                out.push_str(&format!("{marker} {text}\n"));
            }
            SimpleBlock::Paragraph(text) => {
                out.push_str(text);
                out.push('\n');
            }
            SimpleBlock::Link { text, href } => {
                if text == href {
                    out.push_str(href);
                } else {
                    out.push_str(&format!("{text} ({href})"));
                }
                out.push('\n');
            }
            SimpleBlock::Quote(text) => {
                out.push_str(&format!("> {text}\n"));
            }
            SimpleBlock::CodeFence { lang, text } => {
                if let Some(language) = lang {
                    out.push_str(&format!("  [{language}]\n"));
                }
                for line in text.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
            SimpleBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        out.push_str(&format!("{}. {item}\n", index + 1));
                    } else {
                        out.push_str(&format!("* {item}\n"));
                    }
                }
            }
            SimpleBlock::Rule => {
                out.push_str(&"-".repeat(40));
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
        let doc = SimpleDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("# Hello\n"));
        assert!(out.contains("## World\n"));
        assert!(out.contains("=> gemini://example.com Visit\n"));
    }

    #[test]
    fn round_trip_list() {
        let input = "* alpha\n* beta\n* gamma\n";
        let doc = SimpleDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("* alpha\n"));
        assert!(out.contains("* beta\n"));
        assert!(out.contains("* gamma\n"));
    }

    #[test]
    fn round_trip_code_fence() {
        let input = "```rust\nlet x = 1;\n```\n";
        let doc = SimpleDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("```rust\n"));
        assert!(out.contains("let x = 1;\n"));
        assert!(out.contains("```\n"));
    }

    #[test]
    fn link_without_label_uses_url_once() {
        let input = "=> gemini://example.com\n";
        let doc = SimpleDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert_eq!(out.matches("gemini://example.com").count(), 1);
    }

    #[test]
    fn quote_round_trip() {
        let input = "> This is a quote\n";
        let doc = SimpleDocument::from_gemini(input);
        let out = doc.to_gemini();
        assert!(out.contains("> This is a quote\n"));
    }

    #[test]
    fn gophermap_heading_is_info_line() {
        let doc = SimpleDocument::Blocks(vec![SimpleBlock::Heading {
            level: 1,
            text: "My Capsule".to_string(),
        }]);
        let map = doc.to_gophermap("localhost", 70);
        assert!(map.contains("iMy Capsule\t"));
        assert!(map.ends_with(".\r\n"));
    }

    #[test]
    fn gophermap_link_becomes_selector() {
        let doc = SimpleDocument::Blocks(vec![SimpleBlock::Link {
            text: "My Node".to_string(),
            href: "/node/abc".to_string(),
        }]);
        let map = doc.to_gophermap("myhost", 70);
        assert!(map.contains("0My Node\t/node/abc\tmyhost\t70\r\n"));
    }

    #[test]
    fn finger_text_heading_format() {
        let doc = SimpleDocument::Blocks(vec![SimpleBlock::Heading {
            level: 1,
            text: "About Me".to_string(),
        }]);
        let text = doc.to_finger_text();
        assert!(text.contains("# About Me\n"));
    }

    #[test]
    fn finger_text_link_with_url() {
        let doc = SimpleDocument::Blocks(vec![SimpleBlock::Link {
            text: "My Site".to_string(),
            href: "https://example.com".to_string(),
        }]);
        let text = doc.to_finger_text();
        assert!(text.contains("My Site (https://example.com)\n"));
    }
}