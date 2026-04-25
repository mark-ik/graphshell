/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shim that bridges the standalone
//! [`iced-middlenet-viewer`](../../../../crates/iced-middlenet-viewer)
//! crate into the graphshell iced host's message vocabulary.
//!
//! The actual viewer lives in its own crate so it can be developed,
//! tested, and demoed without dragging in graphshell's heavy
//! Servo/webrender dependency tree. This module is the host-side
//! glue: it re-exports `render_scene` for `IcedApp::view` to call,
//! and `Element::map`s the viewer's `MiddlenetViewerEvent` enum
//! into `iced_app::Message::LinkActivated` so the host runtime sees
//! a unified message type. Mirrors the pattern used by
//! `iced_graph_canvas` + `GraphCanvasMessage`.

use iced::Element;

use iced_middlenet_viewer::{MiddlenetViewerEvent, render_scene as crate_render_scene};
use middlenet_engine::render::RenderScene;

use super::iced_app::Message;

/// Render a middlenet `RenderScene` as an iced `Element` whose
/// emitted messages are already mapped into the host's
/// [`Message`] vocabulary. Call site:
/// `IcedApp::view` for nodes whose viewer kind is middlenet.
pub(crate) fn render_scene(scene: &RenderScene) -> Element<'_, Message> {
    crate_render_scene(scene).map(map_event)
}

/// Translate a viewer-emitted event into a host-level [`Message`].
/// Currently a single-arm map (`LinkActivated`); future viewer
/// events (scroll requests, find-in-page, outline navigation) get
/// added here as new variants land in `MiddlenetViewerEvent`.
fn map_event(event: MiddlenetViewerEvent) -> Message {
    match event {
        MiddlenetViewerEvent::LinkActivated(target) => Message::LinkActivated(target),
    }
}
