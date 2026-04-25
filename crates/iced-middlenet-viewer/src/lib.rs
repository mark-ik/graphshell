/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Iced widget that paints middlenet `RenderScene`s.
//!
//! Mirrors the egui dispatch at
//! `registries/viewers/middlenet.rs:902-956` in the main graphshell
//! crate, but lives here as a portable, Servo-free, webrender-free
//! standalone crate. Depends only on `iced` (its own wgpu version)
//! and `middlenet-engine` (CPU-side scene data).
//!
//! The library emits a self-typed [`MiddlenetViewerEvent`] when the
//! user interacts with the rendered scene (currently: link clicks).
//! Hosting apps map the event into their own message type via
//! iced's `Element::map`, mirroring the
//! `graph_canvas::GraphCanvasMessage` pattern used elsewhere in
//! graphshell.

use iced::widget::{column, container, mouse_area, row, rule, text};
use iced::{Color, Element, Font, Length, Pixels};

use middlenet_engine::document::LinkTarget;
use middlenet_engine::render::{
    RenderBlock, RenderBlockKind, RenderScene, RenderTextRun, TextStyle,
};

/// Events the middlenet viewer emits up to its host. Hosts map
/// these into their own message type via `iced::Element::map`.
#[derive(Debug, Clone, PartialEq)]
pub enum MiddlenetViewerEvent {
    /// User clicked a link inside the rendered document.
    LinkActivated(LinkTarget),
}

/// Render a complete middlenet `RenderScene` as an iced `Element`.
///
/// Walks `scene.blocks` in order, dispatching by `RenderBlockKind`
/// to per-kind sub-renderers. Returns an empty column when the
/// scene has no blocks — callers paint nothing in that case.
pub fn render_scene(scene: &RenderScene) -> Element<'_, MiddlenetViewerEvent> {
    let mut col = column![].spacing(BLOCK_SPACING);
    for block in &scene.blocks {
        col = col.push(render_block(block));
    }
    col.width(Length::Fill).into()
}

/// Vertical gap between adjacent blocks, in iced units.
/// Mirrors the egui middlenet viewer's implicit block spacing.
const BLOCK_SPACING: f32 = 8.0;

/// Bullet glyph used for unordered lists. Egui uses the same.
const BULLET: &str = "•";

/// Right-margin reserved before quote text. Aligns with egui's
/// `> {text}` quote formatting.
const QUOTE_PREFIX: &str = "> ";

fn render_block(block: &RenderBlock) -> Element<'_, MiddlenetViewerEvent> {
    match &block.kind {
        RenderBlockKind::Rule => rule::horizontal(1.0).into(),
        RenderBlockKind::CodeFence => render_code_fence(block),
        RenderBlockKind::List { ordered } => render_list(block, *ordered),
        RenderBlockKind::FeedHeader | RenderBlockKind::FeedEntry => render_text_runs(block),
        RenderBlockKind::Heading { .. }
        | RenderBlockKind::Paragraph
        | RenderBlockKind::Link
        | RenderBlockKind::Quote
        | RenderBlockKind::MetadataRow
        | RenderBlockKind::Badge
        | RenderBlockKind::RawSourceNotice => render_text_runs(block),
    }
}

/// Walk a block's `text_runs` and stack them vertically. Inline
/// runs (multiple runs on the same logical line) wait on iced's
/// `Rich`/inline-text APIs landing — for now each run is its own
/// row to match egui's "one run per `ui.label` call" pattern.
fn render_text_runs(block: &RenderBlock) -> Element<'_, MiddlenetViewerEvent> {
    let mut col = column![].spacing(2);
    for run in &block.text_runs {
        col = col.push(render_text_run(run));
    }
    col.into()
}

/// Render a single `RenderTextRun` honoring its `TextStyle`. Runs
/// with a `link_target` become clickable via `mouse_area`, emitting
/// `MiddlenetViewerEvent::LinkActivated(target)` on press.
fn render_text_run(run: &RenderTextRun) -> Element<'_, MiddlenetViewerEvent> {
    let (display_text, font, size, color) = style_for_run(run);

    let label = text(display_text).font(font).size(size).color(color);

    if let Some(target) = run.link_target.as_ref() {
        // mouse_area lets any widget become clickable without
        // pulling in button's styling baggage. We keep the text's
        // visual style (color signals link-ness via TextStyle::Link
        // mapping below); `on_press` emits the activate event.
        mouse_area(label)
            .on_press(MiddlenetViewerEvent::LinkActivated(target.clone()))
            .into()
    } else {
        label.into()
    }
}

/// Map a `TextStyle` to iced text presentation tuple
/// `(display_text, font, size, color)`. Mirrors the egui
/// `RichText` styling at `registries/viewers/middlenet.rs:1024`.
pub fn style_for_run(run: &RenderTextRun) -> (String, Font, Pixels, Color) {
    match run.style {
        TextStyle::Title => (
            run.text.clone(),
            Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            },
            Pixels(26.0),
            Color::WHITE,
        ),
        TextStyle::Heading => (
            run.text.clone(),
            Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            },
            Pixels(20.0),
            Color::WHITE,
        ),
        TextStyle::Quote => (
            format!("{QUOTE_PREFIX}{}", run.text),
            Font {
                style: iced::font::Style::Italic,
                ..Font::DEFAULT
            },
            Pixels(14.0),
            Color::from_rgb(0.667, 0.667, 0.667),
        ),
        TextStyle::Metadata => (
            run.text.clone(),
            Font::DEFAULT,
            Pixels(12.0),
            Color::from_rgb(0.706, 0.706, 0.706),
        ),
        TextStyle::Code => (
            run.text.clone(),
            Font::MONOSPACE,
            Pixels(13.0),
            Color::from_rgb(0.85, 0.85, 0.9),
        ),
        TextStyle::Badge => (
            run.text.clone(),
            Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            },
            Pixels(11.0),
            // Egui's badge color (180, 200, 255) → 0.706, 0.784, 1.000
            Color::from_rgb(0.706, 0.784, 1.0),
        ),
        TextStyle::Link => (
            run.text.clone(),
            Font::DEFAULT,
            Pixels(14.0),
            // Cornflower blue — egui viewer uses (100, 149, 237).
            Color::from_rgb(0.392, 0.584, 0.929),
        ),
        TextStyle::Body => (run.text.clone(), Font::DEFAULT, Pixels(14.0), Color::WHITE),
    }
}

fn render_code_fence(block: &RenderBlock) -> Element<'_, MiddlenetViewerEvent> {
    let body = block
        .text_runs
        .first()
        .map(|run| run.text.clone())
        .unwrap_or_default();

    container(
        text(body)
            .font(Font::MONOSPACE)
            .size(13.0)
            .color(Color::from_rgb(0.85, 0.85, 0.9)),
    )
    .padding(8)
    .width(Length::Fill)
    .into()
}

fn render_list(block: &RenderBlock, _ordered: bool) -> Element<'_, MiddlenetViewerEvent> {
    let body = block
        .text_runs
        .first()
        .map(|run| run.text.as_str())
        .unwrap_or("");

    let mut col = column![].spacing(2);
    for line in body.lines() {
        col = col.push(
            row![
                text(BULLET).size(14.0).color(Color::WHITE),
                text(line.to_string()).size(14.0).color(Color::WHITE),
            ]
            .spacing(6),
        );
    }
    col.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_engine::document::LinkTarget;
    use middlenet_engine::render::RenderRect;

    fn body_run(text: &str) -> RenderTextRun {
        RenderTextRun {
            text: text.to_string(),
            style: TextStyle::Body,
            link_target: None,
        }
    }

    fn rect() -> RenderRect {
        RenderRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 16.0,
        }
    }

    fn block(kind: RenderBlockKind, text: &str) -> RenderBlock {
        RenderBlock {
            kind,
            rect: rect(),
            text_runs: vec![body_run(text)],
            link_target: None,
            indent: 0.0,
        }
    }

    fn empty_scene() -> RenderScene {
        RenderScene {
            blocks: Vec::new(),
            hit_regions: Vec::new(),
            scroll_extent: 0.0,
            outline: Vec::new(),
            diagnostics: Default::default(),
        }
    }

    #[test]
    fn empty_scene_renders_without_panic() {
        let scene = empty_scene();
        let _: Element<'_, MiddlenetViewerEvent> = render_scene(&scene);
    }

    #[test]
    fn each_block_kind_renders_without_panic() {
        let kinds = [
            RenderBlockKind::Heading { level: 1 },
            RenderBlockKind::Paragraph,
            RenderBlockKind::Link,
            RenderBlockKind::Quote,
            RenderBlockKind::CodeFence,
            RenderBlockKind::List { ordered: false },
            RenderBlockKind::List { ordered: true },
            RenderBlockKind::Rule,
            RenderBlockKind::FeedHeader,
            RenderBlockKind::FeedEntry,
            RenderBlockKind::MetadataRow,
            RenderBlockKind::Badge,
            RenderBlockKind::RawSourceNotice,
        ];
        for kind in kinds {
            let scene = RenderScene {
                blocks: vec![block(kind, "sample text")],
                ..empty_scene()
            };
            let _: Element<'_, MiddlenetViewerEvent> = render_scene(&scene);
        }
    }

    #[test]
    fn style_for_run_maps_each_text_style() {
        let title = style_for_run(&RenderTextRun {
            text: "T".into(),
            style: TextStyle::Title,
            link_target: None,
        });
        assert_eq!(title.2, Pixels(26.0));
        assert_eq!(title.1.weight, iced::font::Weight::Bold);

        let code = style_for_run(&RenderTextRun {
            text: "fn main()".into(),
            style: TextStyle::Code,
            link_target: None,
        });
        assert_eq!(code.1, Font::MONOSPACE);
        assert_eq!(code.2, Pixels(13.0));

        let quote = style_for_run(&RenderTextRun {
            text: "wisdom".into(),
            style: TextStyle::Quote,
            link_target: None,
        });
        assert!(
            quote.0.starts_with(QUOTE_PREFIX),
            "quote should be prefixed with '{QUOTE_PREFIX}'; got {:?}",
            quote.0
        );
        assert_eq!(quote.1.style, iced::font::Style::Italic);

        let body = style_for_run(&RenderTextRun {
            text: "plain".into(),
            style: TextStyle::Body,
            link_target: None,
        });
        assert_eq!(body.1, Font::DEFAULT);
        assert_eq!(body.2, Pixels(14.0));
    }

    #[test]
    fn link_run_renders_clickable_text() {
        let run = RenderTextRun {
            text: "click me".into(),
            style: TextStyle::Link,
            link_target: Some(LinkTarget {
                href: "gemini://example.gemini/".into(),
                title: Some("Example".into()),
            }),
        };
        let _: Element<'_, MiddlenetViewerEvent> = render_text_run(&run);
    }

    #[test]
    fn list_block_splits_on_newlines() {
        let scene = RenderScene {
            blocks: vec![RenderBlock {
                kind: RenderBlockKind::List { ordered: false },
                rect: rect(),
                text_runs: vec![body_run("first\nsecond\nthird")],
                link_target: None,
                indent: 0.0,
            }],
            ..empty_scene()
        };
        let _: Element<'_, MiddlenetViewerEvent> = render_scene(&scene);
    }

    /// `MiddlenetViewerEvent` must be portable enough to compare —
    /// downstream hosts assert on it in their parity tests.
    #[test]
    fn event_partial_eq() {
        let a = MiddlenetViewerEvent::LinkActivated(LinkTarget {
            href: "gemini://a/".into(),
            title: None,
        });
        let b = MiddlenetViewerEvent::LinkActivated(LinkTarget {
            href: "gemini://a/".into(),
            title: None,
        });
        let c = MiddlenetViewerEvent::LinkActivated(LinkTarget {
            href: "gemini://different/".into(),
            title: None,
        });
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
