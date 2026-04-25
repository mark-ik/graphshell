/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Demo: iced chrome + wry overlay.
//!
//! Opens an iced window with a status bar at top and a fixed
//! 800×500 overlay region below. On startup, requests the iced
//! window handle, then mounts a `wry::WebView` at the overlay
//! region pointing at example.com. Buttons let you navigate
//! between two URLs and unmount the overlay.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p iced-wry-viewer --example demo
//! ```

use iced::widget::{button, column, container, row, space, text};
use iced::{window, Color, Element, Length, Task};
use iced_wry_viewer::{
    request_window_handle, OverlayRect, WindowHandleOutcome, WryHost,
};

const NODE_ID: u64 = 1;
const URL_HOME: &str = "https://example.com/";
const URL_GEMINI: &str = "https://en.wikipedia.org/wiki/Gemini_(protocol)";

const OVERLAY_RECT: OverlayRect = OverlayRect {
    x: 16.0,
    // Reserve top of window for chrome.
    y: 80.0,
    width: 800.0,
    height: 500.0,
};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| "iced-wry-viewer demo".to_string())
        .run()
}

struct App {
    host: WryHost,
    window_id: window::Id,
    status: String,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let (window_id, open_task) = window::open(window::Settings::default());
        let app = Self {
            host: WryHost::new(),
            window_id,
            status: "requesting window handle…".to_string(),
        };
        // Once the window opens, ask iced for its raw handle and
        // forward it to the host. The handle task fires when the
        // OS has given iced a real winit window.
        let handle_task = request_window_handle(window_id).map(Message::WindowHandle);
        // Discard the open task's window-id payload; we already
        // saved it above.
        (app, open_task.discard().chain(handle_task))
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::WindowHandle(outcome) => {
                let installed = self.host.apply_window_handle_outcome(outcome.clone());
                self.status = if installed {
                    "window handle installed; mounting WebView…".to_string()
                } else {
                    "window handle unavailable; retrying next frame".to_string()
                };
                if installed {
                    let mounted = self.host.mount(NODE_ID, URL_HOME, OVERLAY_RECT);
                    self.status = if mounted {
                        format!("mounted WebView for node {NODE_ID} at {URL_HOME}")
                    } else {
                        "mount returned false; check logs".to_string()
                    };
                } else {
                    // Retry on next frame via a recursive task.
                    return request_window_handle(self.window_id).map(Message::WindowHandle);
                }
                Task::none()
            }
            Message::Navigate(url) => {
                self.host.navigate(NODE_ID, url);
                self.status = format!("navigated to {url}");
                Task::none()
            }
            Message::Unmount => {
                let unmounted = self.host.unmount(NODE_ID);
                self.status = if unmounted {
                    "unmounted webview".into()
                } else {
                    "no webview to unmount".into()
                };
                Task::none()
            }
            Message::Remount => {
                if self.host.mount(NODE_ID, URL_HOME, OVERLAY_RECT) {
                    self.status = format!("re-mounted at {URL_HOME}");
                } else {
                    self.status = "remount failed (no window handle?)".into();
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let toolbar = row![
            button("Home").on_press(Message::Navigate(URL_HOME)),
            button("Gemini info").on_press(Message::Navigate(URL_GEMINI)),
            button("Unmount").on_press(Message::Unmount),
            button("Re-mount").on_press(Message::Remount),
            text(&self.status)
                .size(12.0)
                .color(Color::from_rgb(0.6, 0.6, 0.7)),
        ]
        .spacing(10);

        let body = column![
            text("iced-wry-viewer demo").size(20.0),
            toolbar,
            // The wry overlay paints natively at OVERLAY_RECT; this
            // Space reserves the iced layout slot so chrome below
            // doesn't overlap the WebView region.
            space::vertical().height(Length::Fixed(OVERLAY_RECT.height + 16.0)),
            text("(WebView renders above; iced chrome below.)")
                .size(11.0)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .spacing(8);

        container(body)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    WindowHandle(WindowHandleOutcome),
    Navigate(&'static str),
    Unmount,
    Remount,
}
