/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Standalone demo of the iced-middlenet-viewer.
//!
//! Renders a hand-built `RenderScene` so the viewer can be visually
//! validated without the rest of the graphshell stack. Useful for:
//!
//! - confirming iced widget styling matches what we want
//! - testing link-click → `MiddlenetViewerEvent::LinkActivated` flow
//! - quick iteration on the block dispatch (each `RenderBlockKind`
//!   variant is exercised in the fixture below)
//!
//! Run with:
//!
//! ```bash
//! cargo run -p iced-middlenet-viewer --example demo
//! ```

use iced::widget::{column, container, scrollable, text};
use iced::{Color, Element, Length, Task};
use middlenet_engine::document::LinkTarget;
use middlenet_engine::render::{
    RenderBlock, RenderBlockKind, RenderRect, RenderScene, RenderTextRun, TextStyle,
};

use iced_middlenet_viewer::{MiddlenetViewerEvent, render_scene};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| "iced-middlenet-viewer demo".to_string())
        .run()
}

struct App {
    scene: RenderScene,
    last_link_status: Option<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                scene: fixture_scene(),
                last_link_status: None,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Viewer(MiddlenetViewerEvent::LinkActivated(target)) => {
                self.last_link_status = Some(format!(
                    "link clicked: {} (title: {:?})",
                    target.href, target.title
                ));
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let viewer = render_scene(&self.scene).map(Message::Viewer);

        let status = self
            .last_link_status
            .as_deref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "click a link below to verify event routing".to_string());

        let body = column![
            text("iced-middlenet-viewer demo").size(22.0),
            text(status).size(12.0).color(Color::from_rgb(0.6, 0.6, 0.7)),
            scrollable(viewer).height(Length::Fill),
        ]
        .spacing(12);

        container(body)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    Viewer(MiddlenetViewerEvent),
}

/// Hand-built scene exercising every `RenderBlockKind` variant
/// plus a clickable link. Mirrors what an actual Gemini page
/// might produce, but constructed in code so the demo doesn't
/// need a live network fetch.
fn fixture_scene() -> RenderScene {
    fn rect() -> RenderRect {
        RenderRect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 16.0,
        }
    }
    fn run(text: &str, style: TextStyle) -> RenderTextRun {
        RenderTextRun {
            text: text.to_string(),
            style,
            link_target: None,
        }
    }
    fn link_run(text: &str, href: &str, title: Option<&str>) -> RenderTextRun {
        RenderTextRun {
            text: text.to_string(),
            style: TextStyle::Link,
            link_target: Some(LinkTarget {
                href: href.to_string(),
                title: title.map(|t| t.to_string()),
            }),
        }
    }
    fn block(kind: RenderBlockKind, runs: Vec<RenderTextRun>) -> RenderBlock {
        RenderBlock {
            kind,
            rect: rect(),
            text_runs: runs,
            link_target: None,
            indent: 0.0,
        }
    }

    RenderScene {
        blocks: vec![
            block(
                RenderBlockKind::Heading { level: 1 },
                vec![run("iced-middlenet-viewer demo", TextStyle::Title)],
            ),
            block(
                RenderBlockKind::MetadataRow,
                vec![run("Source: gemini://example.gemini/ · 2026-04-25", TextStyle::Metadata)],
            ),
            block(RenderBlockKind::Rule, vec![]),
            block(
                RenderBlockKind::Paragraph,
                vec![run(
                    "This document exercises every RenderBlockKind variant. \
                     The viewer dispatches each kind to its own iced widget shape, \
                     mirroring the egui middlenet viewer's behavior.",
                    TextStyle::Body,
                )],
            ),
            block(
                RenderBlockKind::Heading { level: 2 },
                vec![run("Section heading", TextStyle::Heading)],
            ),
            block(
                RenderBlockKind::Quote,
                vec![run(
                    "A spatial browser is a map you can arrange, save, and share — \
                     instead of a strip at the top of the window.",
                    TextStyle::Quote,
                )],
            ),
            block(
                RenderBlockKind::List { ordered: false },
                vec![run(
                    "Gemini support\nGopher support\nRSS / Atom feeds\nMarkdown\nPlain text",
                    TextStyle::Body,
                )],
            ),
            block(
                RenderBlockKind::CodeFence,
                vec![run(
                    "fn render_scene(scene: &RenderScene)\n    -> Element<'_, MiddlenetViewerEvent>\n{\n    // ...\n}",
                    TextStyle::Code,
                )],
            ),
            block(
                RenderBlockKind::Link,
                vec![link_run(
                    "Open another Gemini capsule",
                    "gemini://gemini.circumlunar.space/",
                    Some("circumlunar capsule"),
                )],
            ),
            block(
                RenderBlockKind::Badge,
                vec![run("[middlenet]", TextStyle::Badge)],
            ),
            block(
                RenderBlockKind::FeedHeader,
                vec![run("Recent posts", TextStyle::Heading)],
            ),
            block(
                RenderBlockKind::FeedEntry,
                vec![
                    run("2026-04-25 — ", TextStyle::Metadata),
                    link_run(
                        "Why a spatial browser",
                        "gemini://example.gemini/post-1",
                        None,
                    ),
                ],
            ),
            block(
                RenderBlockKind::FeedEntry,
                vec![
                    run("2026-04-20 — ", TextStyle::Metadata),
                    link_run(
                        "Notes on Stylo as a library",
                        "gemini://example.gemini/post-2",
                        None,
                    ),
                ],
            ),
            block(
                RenderBlockKind::RawSourceNotice,
                vec![run(
                    "Showing rendered output. View raw source for full Gemini text.",
                    TextStyle::Metadata,
                )],
            ),
        ],
        hit_regions: Vec::new(),
        scroll_extent: 600.0,
        outline: Vec::new(),
        diagnostics: Default::default(),
    }
}
