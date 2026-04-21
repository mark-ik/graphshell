/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use middlenet_core::document::{LinkTarget, SemanticBlock, SemanticDocument};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    FullPage,
    Card,
    PreviewThumbnail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub body_font_size: f32,
    pub title_font_size: f32,
    pub heading_font_size: f32,
    pub metadata_font_size: f32,
    pub block_spacing: f32,
    pub compact_spacing: f32,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            body_font_size: 16.0,
            title_font_size: 28.0,
            heading_font_size: 20.0,
            metadata_font_size: 13.0,
            block_spacing: 12.0,
            compact_spacing: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderRequest {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub scale_factor: f32,
    pub theme: ThemeTokens,
    pub font_context: Option<String>,
    pub image_resolver: Option<String>,
    pub mode: RenderMode,
}

impl Default for RenderRequest {
    fn default() -> Self {
        Self {
            viewport_width: 720.0,
            viewport_height: 900.0,
            scale_factor: 1.0,
            theme: ThemeTokens::default(),
            font_context: None,
            image_resolver: None,
            mode: RenderMode::FullPage,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RenderRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextStyle {
    Body,
    Title,
    Heading,
    Quote,
    Metadata,
    Code,
    Badge,
    Link,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderTextRun {
    pub text: String,
    pub style: TextStyle,
    pub link_target: Option<LinkTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderBlockKind {
    Heading { level: u8 },
    Paragraph,
    Link,
    Quote,
    CodeFence,
    List { ordered: bool },
    Rule,
    FeedHeader,
    FeedEntry,
    MetadataRow,
    Badge,
    RawSourceNotice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderBlock {
    pub kind: RenderBlockKind,
    pub rect: RenderRect,
    pub text_runs: Vec<RenderTextRun>,
    pub link_target: Option<LinkTarget>,
    pub indent: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitRegion {
    pub rect: RenderRect,
    pub target: LinkTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub block_index: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderDiagnostics {
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderScene {
    pub blocks: Vec<RenderBlock>,
    pub hit_regions: Vec<HitRegion>,
    pub scroll_extent: f32,
    pub outline: Vec<OutlineEntry>,
    pub diagnostics: RenderDiagnostics,
}

pub fn render_document(document: &SemanticDocument, request: &RenderRequest) -> RenderScene {
    let mut blocks = Vec::new();
    let mut hit_regions = Vec::new();
    let mut outline = Vec::new();
    let mut y = 0.0;
    let width = request.viewport_width.max(240.0);
    let spacing = match request.mode {
        RenderMode::FullPage => request.theme.block_spacing,
        RenderMode::Card | RenderMode::PreviewThumbnail => request.theme.compact_spacing,
    };
    let mut diagnostics = RenderDiagnostics::default();

    let iter = match request.mode {
        RenderMode::FullPage => document.blocks.len(),
        RenderMode::Card => document.blocks.len().min(8),
        RenderMode::PreviewThumbnail => document.blocks.len().min(4),
    };

    for (index, block) in document.blocks.iter().take(iter).enumerate() {
        let (kind, text_runs, link_target, indent, height, outline_entry) =
            describe_block(block, request, width);
        let rect = RenderRect {
            x: indent,
            y,
            width: (width - indent).max(120.0),
            height,
        };

        if let Some(target) = link_target.clone() {
            hit_regions.push(HitRegion {
                rect,
                target: target.clone(),
            });
        }

        if let Some(entry) = outline_entry {
            outline.push(OutlineEntry {
                level: entry.0,
                title: entry.1,
                block_index: index,
            });
        }

        if matches!(block, SemanticBlock::RawSourceNotice { .. }) {
            diagnostics
                .messages
                .push("Rendered raw-source notice block".to_string());
        }

        blocks.push(RenderBlock {
            kind,
            rect,
            text_runs,
            link_target,
            indent,
        });

        y += height + spacing;
    }

    if matches!(request.mode, RenderMode::PreviewThumbnail) && document.blocks.len() > iter {
        diagnostics
            .messages
            .push("Preview scene truncated to thumbnail budget".to_string());
    }

    RenderScene {
        blocks,
        hit_regions,
        scroll_extent: y.max(request.viewport_height),
        outline,
        diagnostics,
    }
}

fn describe_block(
    block: &SemanticBlock,
    request: &RenderRequest,
    width: f32,
) -> (
    RenderBlockKind,
    Vec<RenderTextRun>,
    Option<LinkTarget>,
    f32,
    f32,
    Option<(u8, String)>,
) {
    match block {
        SemanticBlock::Heading { level, text } => {
            let style = if *level == 1 {
                TextStyle::Title
            } else {
                TextStyle::Heading
            };
            let font = if *level == 1 {
                request.theme.title_font_size
            } else {
                request.theme.heading_font_size
            };
            let height = text_height(text, width, font);
            (
                RenderBlockKind::Heading { level: *level },
                vec![RenderTextRun {
                    text: text.clone(),
                    style,
                    link_target: None,
                }],
                None,
                0.0,
                height,
                Some((*level, text.clone())),
            )
        }
        SemanticBlock::Paragraph(text) => (
            RenderBlockKind::Paragraph,
            vec![RenderTextRun {
                text: text.clone(),
                style: TextStyle::Body,
                link_target: None,
            }],
            None,
            0.0,
            text_height(text, width, request.theme.body_font_size),
            None,
        ),
        SemanticBlock::Link { text, target } => (
            RenderBlockKind::Link,
            vec![RenderTextRun {
                text: text.clone(),
                style: TextStyle::Link,
                link_target: Some(target.clone()),
            }],
            Some(target.clone()),
            0.0,
            text_height(text, width, request.theme.body_font_size),
            None,
        ),
        SemanticBlock::Quote(text) => (
            RenderBlockKind::Quote,
            vec![RenderTextRun {
                text: text.clone(),
                style: TextStyle::Quote,
                link_target: None,
            }],
            None,
            12.0,
            text_height(text, width - 12.0, request.theme.body_font_size),
            None,
        ),
        SemanticBlock::CodeFence { text, .. } => (
            RenderBlockKind::CodeFence,
            vec![RenderTextRun {
                text: text.clone(),
                style: TextStyle::Code,
                link_target: None,
            }],
            None,
            12.0,
            text_height(text, width - 12.0, request.theme.metadata_font_size) + 8.0,
            None,
        ),
        SemanticBlock::List { ordered, items } => {
            let joined = items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    if *ordered {
                        format!("{}. {}", index + 1, item)
                    } else {
                        format!("• {item}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            (
                RenderBlockKind::List { ordered: *ordered },
                vec![RenderTextRun {
                    text: joined.clone(),
                    style: TextStyle::Body,
                    link_target: None,
                }],
                None,
                16.0,
                text_height(&joined, width - 16.0, request.theme.body_font_size),
                None,
            )
        }
        SemanticBlock::Rule => (RenderBlockKind::Rule, Vec::new(), None, 0.0, 8.0, None),
        SemanticBlock::FeedHeader {
            title,
            subtitle,
            summary,
            source_link,
        } => {
            let mut runs = vec![RenderTextRun {
                text: title.clone(),
                style: TextStyle::Title,
                link_target: None,
            }];
            if let Some(subtitle) = subtitle {
                runs.push(RenderTextRun {
                    text: subtitle.clone(),
                    style: TextStyle::Heading,
                    link_target: None,
                });
            }
            if let Some(summary) = summary {
                runs.push(RenderTextRun {
                    text: summary.clone(),
                    style: TextStyle::Body,
                    link_target: None,
                });
            }
            if let Some(link) = source_link {
                runs.push(RenderTextRun {
                    text: link
                        .title
                        .clone()
                        .unwrap_or_else(|| "Open feed source".to_string()),
                    style: TextStyle::Link,
                    link_target: Some(link.clone()),
                });
            }
            let height = runs
                .iter()
                .map(|run| text_height(&run.text, width, font_size_for_style(&run.style, request)))
                .sum::<f32>()
                + 8.0;
            (
                RenderBlockKind::FeedHeader,
                runs,
                source_link.clone(),
                0.0,
                height,
                Some((1, title.clone())),
            )
        }
        SemanticBlock::FeedEntry {
            title,
            date,
            summary,
            article_link,
            source_link,
        } => {
            let mut runs = vec![RenderTextRun {
                text: title.clone(),
                style: TextStyle::Heading,
                link_target: None,
            }];
            if let Some(date) = date {
                runs.push(RenderTextRun {
                    text: date.clone(),
                    style: TextStyle::Metadata,
                    link_target: None,
                });
            }
            if let Some(summary) = summary {
                runs.push(RenderTextRun {
                    text: summary.clone(),
                    style: TextStyle::Body,
                    link_target: None,
                });
            }
            if let Some(link) = article_link {
                runs.push(RenderTextRun {
                    text: link
                        .title
                        .clone()
                        .unwrap_or_else(|| "Open article".to_string()),
                    style: TextStyle::Link,
                    link_target: Some(link.clone()),
                });
            }
            if let Some(link) = source_link {
                runs.push(RenderTextRun {
                    text: link
                        .title
                        .clone()
                        .unwrap_or_else(|| "Open source".to_string()),
                    style: TextStyle::Link,
                    link_target: Some(link.clone()),
                });
            }
            let height = runs
                .iter()
                .map(|run| {
                    text_height(
                        &run.text,
                        width - 8.0,
                        font_size_for_style(&run.style, request),
                    )
                })
                .sum::<f32>()
                + 8.0;
            (
                RenderBlockKind::FeedEntry,
                runs,
                article_link.clone().or_else(|| source_link.clone()),
                8.0,
                height,
                Some((2, title.clone())),
            )
        }
        SemanticBlock::MetadataRow { label, value } => (
            RenderBlockKind::MetadataRow,
            vec![RenderTextRun {
                text: format!("{label}: {value}"),
                style: TextStyle::Metadata,
                link_target: None,
            }],
            None,
            0.0,
            text_height(value, width, request.theme.metadata_font_size),
            None,
        ),
        SemanticBlock::Badge { text } => (
            RenderBlockKind::Badge,
            vec![RenderTextRun {
                text: text.clone(),
                style: TextStyle::Badge,
                link_target: None,
            }],
            None,
            0.0,
            text_height(text, width, request.theme.metadata_font_size),
            None,
        ),
        SemanticBlock::RawSourceNotice { note } => (
            RenderBlockKind::RawSourceNotice,
            vec![RenderTextRun {
                text: note.clone(),
                style: TextStyle::Metadata,
                link_target: None,
            }],
            None,
            0.0,
            text_height(note, width, request.theme.metadata_font_size),
            None,
        ),
    }
}

fn font_size_for_style(style: &TextStyle, request: &RenderRequest) -> f32 {
    match style {
        TextStyle::Title => request.theme.title_font_size,
        TextStyle::Heading => request.theme.heading_font_size,
        TextStyle::Metadata | TextStyle::Code | TextStyle::Badge => {
            request.theme.metadata_font_size
        }
        TextStyle::Body | TextStyle::Quote | TextStyle::Link => request.theme.body_font_size,
    }
}

fn text_height(text: &str, width: f32, font_size: f32) -> f32 {
    let effective_width = width.max(120.0);
    let chars_per_line = ((effective_width / (font_size * 0.56)).floor() as usize).max(12);
    let line_count = text
        .lines()
        .map(|line| line.chars().count().max(1).div_ceil(chars_per_line))
        .sum::<usize>()
        .max(1);
    (line_count as f32 * (font_size * 1.35)).max(font_size + 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::document::{DocumentMeta, DocumentProvenance};
    use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};

    fn feed_document() -> SemanticDocument {
        let source = MiddleNetSource::new(MiddleNetContentKind::Rss)
            .with_uri("https://example.com/feed.xml")
            .with_title_hint("Example Feed");
        SemanticDocument::new(
            DocumentMeta::for_source(&source),
            DocumentProvenance::for_source(&source),
            vec![
                SemanticBlock::FeedHeader {
                    title: "Example Feed".to_string(),
                    subtitle: Some("Recent notes".to_string()),
                    summary: Some("Updates from Graphshell".to_string()),
                    source_link: Some(
                        LinkTarget::new("https://example.com/feed.xml")
                            .with_title("Open feed source"),
                    ),
                },
                SemanticBlock::FeedEntry {
                    title: "Entry one".to_string(),
                    date: Some("2026-04-20".to_string()),
                    summary: Some("A fairly compact summary".to_string()),
                    article_link: Some(
                        LinkTarget::new("https://example.com/posts/1").with_title("Open article"),
                    ),
                    source_link: None,
                },
            ],
        )
    }

    #[test]
    fn semantic_feed_document_produces_render_scene() {
        let scene = render_document(&feed_document(), &RenderRequest::default());

        assert_eq!(scene.blocks.len(), 2);
        assert!(scene.scroll_extent >= 900.0);
        assert!(!scene.outline.is_empty());
    }

    #[test]
    fn hit_regions_follow_link_blocks() {
        let scene = render_document(&feed_document(), &RenderRequest::default());

        assert!(
            scene
                .hit_regions
                .iter()
                .any(|region| region.target.href == "https://example.com/posts/1")
        );
    }

    #[test]
    fn preview_mode_truncates_scene() {
        let mut document = feed_document();
        document
            .blocks
            .push(SemanticBlock::Paragraph("Trailing context".to_string()));
        document.blocks.push(SemanticBlock::Paragraph(
            "More trailing context".to_string(),
        ));
        document.blocks.push(SemanticBlock::Paragraph(
            "Even more trailing context".to_string(),
        ));

        let scene = render_document(
            &document,
            &RenderRequest {
                mode: RenderMode::PreviewThumbnail,
                ..RenderRequest::default()
            },
        );

        assert!(scene.blocks.len() <= 4);
        assert!(
            scene
                .diagnostics
                .messages
                .iter()
                .any(|message| message.contains("truncated"))
        );
    }
}
