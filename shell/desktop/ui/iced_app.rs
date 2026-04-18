/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced `Program` surface around [`IcedHost`] — M5.2.
//!
//! Third layer in the iced host stack:
//!
//! 1. [`GraphshellRuntime`] — host-neutral state, shared with `EguiHost`.
//! 2. [`super::iced_host::IcedHost`] — iced-side adapter around the runtime.
//! 3. [`IcedApp`] *(this module)* — iced `Program`-shaped type iced's event
//!    loop actually drives.
//!
//! **Scope**: renders a blank placeholder window with a status label.
//! The view does not yet consume `FrameViewModel`, and no input events
//! drive `IcedHost::tick_with_input`. Those wire up in M5.3 (event
//! translation) and M5.4 (first real surface — graph canvas).
//!
//! The `run_application()` helper builds an `iced::Application` configured
//! with our update/view functions. It is not yet invoked from `main.rs` —
//! M5.1/M5.2 only prove the iced `Program` bundle compiles; a second
//! desktop entry point lands when the first real surface renders.

use iced::widget::{canvas, column, container, text};
use iced::{Element, Length, Task};

use crate::shell::desktop::ui::frame_model::FrameHostInput;
use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
use crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasProgram;
use crate::shell::desktop::ui::iced_host::IcedHost;

/// App-level state held across iced frames.
///
/// Owns the `IcedHost` adapter (which in turn owns the shared runtime).
/// Every iced frame drives `IcedHost::tick_with_input` via a synthetic
/// `Tick` message; later steps will route the real event stream instead.
pub(crate) struct IcedApp {
    pub(crate) host: IcedHost,
}

/// Messages iced pushes into `IcedApp::update`.
///
/// Intentionally small during M5 skeleton work. M5.3 adds translated
/// `HostEvent`s alongside `Tick` so the runtime's tick path sees live
/// input; M5.4 adds surface-specific messages.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Frame pulse — used to drive `IcedHost::tick_with_input` without
    /// requiring real input events yet.
    Tick,
}

impl IcedApp {
    /// Construct an app whose `IcedHost` wraps the supplied runtime.
    pub(crate) fn with_runtime(runtime: GraphshellRuntime) -> Self {
        Self {
            host: IcedHost { runtime },
        }
    }

    fn title(&self) -> String {
        "Graphshell — iced host (M5 skeleton)".to_string()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                let input = FrameHostInput::default();
                let _view_model = self.host.tick_with_input(&input);
                // todo(m5.3): feed the returned view-model into the
                // `view` function through app state rather than discarding
                // it. Requires growing `IcedApp` to cache the last
                // projection so the view function can read it.
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // M5.4: first real surface. Snapshot the shared graph and render
        // it on an iced canvas. Interaction / proper CanvasBackend
        // integration lands in follow-on work.
        let program = GraphCanvasProgram::from_graph_app(&self.host.runtime.graph_app);
        let graph = canvas(program).width(Length::Fill).height(Length::Fill);

        let body = column![
            text("Graphshell — iced host (graph canvas)").size(20),
            graph,
        ]
        .spacing(8);

        container(body)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Wire up an `iced::Application` around `IcedApp`.
///
/// Invoked from `cli::main` when `--iced` / `GRAPHSHELL_ICED=1` is set.
/// The runtime passed in is the minimal bring-up variant (no Servo, no
/// persistence). Follow-on work: swap in a production runtime builder
/// once the host boundary owns webview + persistence init.
///
/// iced 0.14 note: the builder signature changed from
/// `application(title, update, view).run_with(boot)` to
/// `application(boot, update, view).title(title).run()`. `boot` is
/// invoked once by iced to produce `(State, Task<Message>)`; we use
/// an `Option::take` so the captured `runtime` is moved into the
/// constructed `IcedApp` exactly once.
pub(crate) fn run_application(runtime: GraphshellRuntime) -> iced::Result {
    // iced 0.14's `BootFn` requires `Fn` (not `FnOnce`), so a plain
    // `Option::take` on a captured `mut` binding won't compile — the
    // closure would be `FnMut`. `RefCell` gives us interior mutability
    // while the closure itself captures only a shared reference, which
    // satisfies `Fn`. In practice the boot closure is invoked exactly
    // once by iced; the second-call panic is defensive.
    let runtime_slot = std::cell::RefCell::new(Some(runtime));
    iced::application(
        move || {
            let runtime = runtime_slot
                .borrow_mut()
                .take()
                .expect("iced application boot closure called more than once");
            (IcedApp::with_runtime(runtime), Task::none())
        },
        IcedApp::update,
        IcedApp::view,
    )
    .title(IcedApp::title)
    .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iced_app_tick_drives_runtime() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // A Tick message should produce no follow-up tasks and leave
        // the shared runtime intact (tick is idempotent given empty input).
        let _task = app.update(Message::Tick);

        // The view function should produce an element without panicking.
        let _element = app.view();
    }
}
