/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced `Program` surface around [`IcedHost`] — M5.2 + M5.3.
//!
//! Third layer in the iced host stack:
//!
//! 1. [`GraphshellRuntime`] — host-neutral state, shared with `EguiHost`.
//! 2. [`super::iced_host::IcedHost`] — iced-side adapter around the runtime.
//! 3. [`IcedApp`] *(this module)* — iced `Program`-shaped type iced's event
//!    loop actually drives.
//!
//! **Scope (M5.4c)**: real event subscription is wired. iced's event loop
//! pushes raw `iced::Event`s via `subscription()`; `update` translates
//! each through `iced_events::from_iced_event` into a `HostEvent` and
//! immediately drives `IcedHost::tick_with_input` with a populated
//! `FrameHostInput`. Events without a host-neutral equivalent are
//! dropped at the translation boundary (matches the egui host's
//! `HostEvent::from_egui_event` behavior).
//!
//! Batched ticks (multiple events per frame) and view-model consumption
//! in `view` are follow-on slices.

use euclid::default::Vector2D;
use graph_canvas::camera::CanvasCamera;
use iced::widget::{canvas, column, container, text, text_input};
use iced::{Element, Length, Subscription, Task};

/// Stable widget id for the toolbar location text input. Used by the
/// Ctrl+L hotkey handler to address the widget via
/// `iced::widget::operation::focus`. Any future iced widget that
/// wants programmatic focus gets a similar named id rather than a
/// freshly-generated one, so the id is portable across `view` rebuilds.
const LOCATION_INPUT_ID: &str = "graphshell:location_bar";

use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
use crate::shell::desktop::ui::iced_graph_canvas::{
    GraphCanvasProgram, from_graph_app as graph_canvas_from_app,
};
use crate::shell::desktop::ui::iced_host::IcedHost;
use graphshell_core::host_event::HostEvent;
use graphshell_runtime::{FrameHostInput, FrameViewModel, ToastSeverity};

/// App-level state held across iced frames.
///
/// Owns the `IcedHost` adapter (which in turn owns the shared runtime)
/// plus the most recent `FrameViewModel` the runtime produced — cached
/// so the next `view` call can read projected state (focus, toolbar,
/// etc.) without re-running `tick`.
pub(crate) struct IcedApp {
    pub(crate) host: IcedHost,
    /// Last `FrameViewModel` produced by `runtime.tick`. `None` before
    /// the first tick; populated lazily after the first real input or
    /// explicit `Tick` message.
    pub(crate) last_view_model: Option<FrameViewModel>,
    /// Uncommitted toolbar location text the user is currently typing.
    /// `None` means "display `last_view_model.toolbar.location`"; `Some`
    /// means "the user has typed, show this instead." Cleared on
    /// `LocationSubmitted` so the text input resumes mirroring the
    /// runtime's projected location once a submit lands.
    pub(crate) location_draft: Option<String>,
}

/// Messages iced pushes into `IcedApp::update`.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Frame pulse with no input — drives `tick` against an empty
    /// `FrameHostInput` so the runtime can advance time-based state
    /// (focus-ring fades, etc.) even in the absence of user input.
    Tick,
    /// Raw iced event from the event subscription. Translated into a
    /// `HostEvent` via `iced_events::from_iced_event` and ticked
    /// through the runtime immediately. This message path also caches
    /// cursor-position and modifier state onto `IcedHost` so the
    /// `HostInputPort` getters surface live values at tick time.
    IcedEvent(iced::Event),
    /// Camera state mutated in the graph canvas. Published by
    /// `GraphCanvasProgram::update` after wheel-zoom or drag-pan
    /// applies to `GraphCanvasState.camera`. `update` persists the
    /// new values into the runtime's per-view camera map so other
    /// surfaces (or the egui host during overlap) see the same
    /// camera state.
    CameraChanged { pan: Vector2D<f32>, zoom: f32 },
    /// Toolbar location text edited. Updates `location_draft` only —
    /// no tick runs, no runtime mutation. The draft reverts to
    /// mirroring the view-model on `LocationSubmitted`.
    LocationEdited(String),
    /// Toolbar location submitted (Enter pressed). For now, enqueues
    /// an ack toast and clears the draft — actual navigation requires
    /// extending `FrameHostInput` with an `intents` channel so host
    /// adapters can route `GraphIntent::CreateNodeAtUrl` into the
    /// runtime without violating §12.17's no-direct-mutation contract.
    /// Tracked as a follow-on in the 2026-04-24 execution log.
    LocationSubmitted,
    /// User clicked a link inside a middlenet-rendered document
    /// (M1.4 of the content-surface scoping doc). Routes through the
    /// same `HostIntent::CreateNodeAtUrl` path the toolbar submit
    /// uses, so a link click creates a new graph node — spatial-
    /// browsing semantics. Position defaults to origin; force-directed
    /// physics will reposition.
    LinkActivated(middlenet_engine::document::LinkTarget),
}

impl IcedApp {
    /// Construct an app whose `IcedHost` wraps the supplied runtime.
    pub(crate) fn with_runtime(runtime: GraphshellRuntime) -> Self {
        Self {
            host: IcedHost::with_runtime(runtime),
            last_view_model: None,
            location_draft: None,
        }
    }

    /// Text shown in the toolbar text input. Drafts take precedence
    /// over the view-model's projected location so the user sees
    /// their typing in real time; cleared on submit so the display
    /// resumes mirroring the runtime.
    fn location_value(&self) -> String {
        if let Some(draft) = self.location_draft.as_ref() {
            return draft.clone();
        }
        self.last_view_model
            .as_ref()
            .map(|vm| vm.toolbar.location.clone())
            .unwrap_or_default()
    }

    fn title(&self) -> String {
        "Graphshell — iced host (M5)".to_string()
    }

    /// Drive one tick of the runtime with the supplied host-neutral
    /// events. Extracted so both `Message::Tick` and
    /// `Message::IcedEvent` converge on the same tick path.
    fn tick_with_events(&mut self, events: Vec<HostEvent>) {
        let had_input_events = !events.is_empty();
        let input = FrameHostInput {
            events,
            had_input_events,
            ..FrameHostInput::default()
        };
        let view_model = self.host.tick_with_input(&input);
        self.last_view_model = Some(view_model);
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.tick_with_events(Vec::new());
                Task::none()
            }
            Message::IcedEvent(event) => {
                // Cache cursor + modifier state on the host before
                // ticking so `HostInputPort::pointer_hover_position`
                // and `HostInputPort::modifiers` surface live values
                // inside this tick. Runs regardless of whether the
                // event translates into a `HostEvent`, because these
                // are state *snapshots*, not events.
                self.cache_host_input_state(&event);

                // App-level hotkeys that don't belong in the runtime's
                // `HostEvent` vocabulary — e.g. Ctrl+L is a host-chrome
                // concern (focus the toolbar), not a graph-runtime
                // concern. Intercept before translating, and return
                // the focus task in place of the runtime tick.
                if is_focus_location_hotkey(&event) {
                    return iced::widget::operation::focus(iced::widget::Id::new(
                        LOCATION_INPUT_ID,
                    ));
                }

                // Translate; drop events with no host-neutral equivalent
                // (CursorEntered/Left, unsupported keys, IME, etc.).
                // Only tick if something translated — otherwise the
                // runtime sees a spurious empty-input tick per iced
                // event, which wastes work.
                let events: Vec<HostEvent> = super::iced_events::from_iced_event(&event)
                    .into_iter()
                    .collect();
                if !events.is_empty() {
                    self.tick_with_events(events);
                }
                Task::none()
            }
            Message::CameraChanged { pan, zoom } => {
                // Persist the canvas-captured camera into the runtime's
                // per-view camera map so it survives canvas-widget
                // destruction/recreation and so other surfaces (and
                // the egui host during overlap) see the same camera.
                let view_id = self.host.view_id;
                let entry = self
                    .host
                    .runtime
                    .graph_app
                    .workspace
                    .graph_runtime
                    .canvas_cameras
                    .entry(view_id)
                    .or_insert_with(CanvasCamera::default);
                entry.pan = pan;
                entry.zoom = zoom;
                entry.pan_velocity = Vector2D::zero();
                Task::none()
            }
            Message::LocationEdited(new_value) => {
                // Uncommitted typing — update the draft only, no tick.
                self.location_draft = Some(new_value);
                Task::none()
            }
            Message::LocationSubmitted => {
                // §12.17 sanctioned path: host produces a `HostIntent`,
                // runtime applies it through the reducer. We queue
                // the intent on `IcedHost::pending_host_intents` and
                // immediately tick so the new node lands in the same
                // frame the submit happened in.
                let submitted = self.location_draft.take().unwrap_or_default();
                if !submitted.is_empty() {
                    self.queue_create_node_at_url(submitted.clone());
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Success,
                        message: format!("opened: {submitted}"),
                        duration: None,
                    });
                }
                Task::none()
            }
            Message::LinkActivated(target) => {
                // Link click inside a rendered middlenet document →
                // create a new graph node at the target URL. Same
                // `HostIntent::CreateNodeAtUrl` path the toolbar
                // submit uses; spatial-browsing semantics (links
                // open as new nodes, not navigate-in-place).
                let href = target.href.clone();
                if !href.is_empty() {
                    self.queue_create_node_at_url(href.clone());
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("link → {href}"),
                        duration: None,
                    });
                }
                Task::none()
            }
        }
    }

    /// Queue a `HostIntent::CreateNodeAtUrl` for the next tick and
    /// drive it. Shared between `LocationSubmitted` and `LinkActivated`
    /// so both routes flow through the same sanctioned-writes path.
    fn queue_create_node_at_url(&mut self, url: String) {
        self.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::CreateNodeAtUrl {
                url,
                position: graphshell_core::geometry::PortablePoint::new(0.0, 0.0),
            },
        );
        // Tick immediately so the runtime drains the queued intent
        // in the same frame. Empty `events` is fine — the tick
        // loop also routes `host_intents`.
        self.tick_with_events(Vec::new());
    }

    /// Update `IcedHost.cursor_position` / `IcedHost.modifiers` from
    /// an incoming iced event so the `HostInputPort` getters surface
    /// live values on the next tick. Silently ignores events that
    /// don't carry either payload.
    fn cache_host_input_state(&mut self, event: &iced::Event) {
        match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                self.host.cursor_position = Some(*position);
            }
            iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                self.host.cursor_position = None;
            }
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                self.host.modifiers = *mods;
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { modifiers, .. })
            | iced::Event::Keyboard(iced::keyboard::Event::KeyReleased { modifiers, .. }) => {
                // Keep `modifiers` in sync with every keyboard event;
                // iced emits `ModifiersChanged` on transitions but also
                // surfaces the current modifiers on every key event.
                self.host.modifiers = *modifiers;
            }
            _ => {}
        }
    }

    /// Subscribe to iced's native event stream. Every event iced
    /// dispatches (mouse, keyboard, window, touch, IME) is delivered
    /// to `update` as `Message::IcedEvent(...)` and flows through the
    /// same `iced_events::from_iced_event` translation path the parity
    /// tests exercise.
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen().map(Message::IcedEvent)
    }

    fn view(&self) -> Element<'_, Message> {
        // Render chrome from the cached `FrameViewModel`. `view` is a
        // pure function of app state, and the view-model is exactly
        // that — iced reads the portable projections directly without
        // any painter-port plumbing.
        let location_value = self.location_value();
        let location_input: Element<'_, Message> = text_input("Enter URL…", &location_value)
            .id(iced::widget::Id::new(LOCATION_INPUT_ID))
            .on_input(Message::LocationEdited)
            .on_submit(Message::LocationSubmitted)
            .size(14)
            .padding(4)
            .width(Length::Fill)
            .into();

        let toolbar_row = if let Some(vm) = self.last_view_model.as_ref() {
            let nav_hint = format!(
                "back:{}  fwd:{}",
                if vm.toolbar.can_go_back { "✓" } else { "·" },
                if vm.toolbar.can_go_forward {
                    "✓"
                } else {
                    "·"
                },
            );
            let focus_hint = if vm.focus.graph_surface_focused {
                "focus: graph"
            } else {
                "focus: chrome"
            };
            iced::widget::row![
                location_input,
                text(nav_hint).size(12),
                text(focus_hint).size(12),
            ]
            .spacing(16)
            .align_y(iced::Alignment::Center)
        } else {
            iced::widget::row![location_input, text("waiting for first tick…").size(12),]
                .spacing(16)
                .align_y(iced::Alignment::Center)
        };

        // Graph canvas publishes `GraphCanvasMessage` — map up into our
        // app-level `Message` via `Element::map` so the iced-idiomatic
        // child-message pattern round-trips cleanly.
        let program = graph_canvas_from_app(&self.host.runtime.graph_app, self.host.view_id);
        // `program` typed as `GraphCanvasProgram` re-exported from the
        // standalone viewer crate via the shim module.
        let _: &GraphCanvasProgram = &program;
        let graph: Element<'_, super::iced_graph_canvas::GraphCanvasMessage> = canvas(program)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        let graph: Element<'_, Message> = graph.map(|gcm| match gcm {
            super::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
                Message::CameraChanged { pan, zoom }
            }
        });

        // Toast stack rendered at the bottom of the body column.
        // Currently has no auto-dismiss — toasts persist until
        // `IcedHost`'s bounded-queue policy drops the oldest. A
        // subscription-timer-driven auto-dismiss is a follow-on.
        let toast_stack = render_toast_stack(&self.host.toast_queue);

        let body = column![
            text("Graphshell — iced host").size(20),
            toolbar_row,
            graph,
            toast_stack,
        ]
        .spacing(8);

        container(body)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Is this iced event the "focus the location bar" hotkey?
/// Matches browser convention: Ctrl+L (Cmd+L on macOS, which iced's
/// `Modifiers::command()` abstracts over). Consumed at the app level —
/// never reaches the runtime's `HostEvent` translation.
fn is_focus_location_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            let is_l = c.as_ref().eq_ignore_ascii_case("l");
            is_l && modifiers.command()
        }
        _ => false,
    }
}

/// Render the host's toast queue as a stack of severity-prefixed rows.
/// Kept module-private; `view` calls it directly.
fn render_toast_stack(
    toasts: &[graphshell_runtime::ToastSpec],
) -> iced::widget::Column<'_, Message> {
    if toasts.is_empty() {
        return iced::widget::column![];
    }
    let mut col = iced::widget::column![].spacing(4);
    for toast in toasts {
        let severity_tag = match toast.severity {
            ToastSeverity::Info => "ℹ",
            ToastSeverity::Success => "✓",
            ToastSeverity::Warning => "⚠",
            ToastSeverity::Error => "✗",
        };
        col = col.push(text(format!("{severity_tag} {}", toast.message)).size(12));
    }
    col
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
    .subscription(IcedApp::subscription)
    .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iced_app_tick_drives_runtime() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(
            app.last_view_model.is_none(),
            "view-model cache should start empty",
        );

        // A Tick message should produce no follow-up tasks and leave
        // the shared runtime intact (tick is idempotent given empty input).
        let _task = app.update(Message::Tick);

        assert!(
            app.last_view_model.is_some(),
            "Tick should populate the view-model cache",
        );

        // The view function should produce an element without panicking.
        let _element = app.view();
    }

    /// End-to-end event wiring: an iced event flows through `update`,
    /// gets translated by `iced_events::from_iced_event`, and drives
    /// `runtime.tick` with a populated `FrameHostInput`. The resulting
    /// `FrameViewModel` is cached on `IcedApp` so `view` can consume it.
    #[test]
    fn iced_event_drives_runtime_tick_via_update() {
        use iced::Point;
        use iced::mouse;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Cursor move → PointerMoved via iced_events::from_iced_event.
        let event = iced::Event::Mouse(mouse::Event::CursorMoved {
            position: Point { x: 42.0, y: 24.0 },
        });
        let _task = app.update(Message::IcedEvent(event));

        assert!(
            app.last_view_model.is_some(),
            "translated iced event should have driven a runtime tick",
        );
    }

    /// Events with no host-neutral translation (e.g. `CursorEntered`)
    /// must NOT drive a spurious tick — otherwise the runtime eats
    /// extra work per iced event.
    #[test]
    fn untranslatable_iced_event_does_not_tick() {
        use iced::mouse;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let event = iced::Event::Mouse(mouse::Event::CursorEntered);
        let _task = app.update(Message::IcedEvent(event));

        assert!(
            app.last_view_model.is_none(),
            "untranslatable event should be dropped without ticking the runtime",
        );
    }

    /// Canvas → runtime camera round-trip: a `CameraChanged` message
    /// (published by `GraphCanvasProgram::update`) must land on the
    /// runtime's per-view camera map keyed by `IcedHost::view_id`.
    #[test]
    fn camera_changed_persists_to_runtime_canvas_cameras() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let view_id = app.host.view_id;

        let pan = Vector2D::new(42.0, -17.0);
        let zoom = 1.75;
        let _task = app.update(Message::CameraChanged { pan, zoom });

        let camera = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .canvas_cameras
            .get(&view_id)
            .expect("camera should be persisted under the host view_id");
        assert_eq!(camera.pan, pan);
        assert_eq!(camera.zoom, zoom);
        assert_eq!(camera.pan_velocity, Vector2D::zero());
    }

    /// `LocationEdited` updates the draft without ticking — typing
    /// is zero-cost at the runtime level. The view-model cache is
    /// untouched (no runtime tick ran).
    #[test]
    fn location_edited_updates_draft_without_ticking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.location_draft.is_none(), "draft starts empty");
        assert!(app.last_view_model.is_none(), "no tick has run");

        let _task = app.update(Message::LocationEdited("https://exa".into()));
        assert_eq!(app.location_draft.as_deref(), Some("https://exa"));
        assert!(
            app.last_view_model.is_none(),
            "typing must not tick the runtime",
        );

        let _task = app.update(Message::LocationEdited("https://example.com".into()));
        assert_eq!(
            app.location_draft.as_deref(),
            Some("https://example.com"),
            "subsequent edits replace the draft",
        );
    }

    /// `LocationSubmitted` clears the draft, enqueues an ack toast,
    /// and — the important bit post-intent-routing — actually creates
    /// a graph node via the sanctioned `HostIntent` path. The runtime
    /// translates the intent to `add_node_and_sync` during the tick
    /// that `LocationSubmitted` triggers.
    #[test]
    fn location_submitted_clears_draft_and_creates_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::LocationEdited("https://submit.test/".into()));
        let _ = app.update(Message::LocationSubmitted);

        assert!(app.location_draft.is_none(), "submit should clear draft");
        assert_eq!(
            app.host.toast_queue.len(),
            1,
            "submit should enqueue an ack toast",
        );
        assert!(
            app.host.toast_queue[0]
                .message
                .contains("https://submit.test/"),
            "toast should reference the submitted URL; got {:?}",
            app.host.toast_queue[0].message,
        );

        // Runtime drained the queued `HostIntent::CreateNodeAtUrl`
        // during the tick `LocationSubmitted` triggered, so the node
        // is visible in the domain graph.
        let nodes_after = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(
            nodes_after,
            nodes_before + 1,
            "submit should add exactly one node via HostIntent routing",
        );
        // The new node's URL matches what was submitted.
        assert!(
            app.host
                .runtime
                .graph_app
                .domain_graph()
                .nodes()
                .any(|(_, n)| n.url() == "https://submit.test/"),
            "new node's URL should match the submitted value",
        );

        // Host's pending-intent queue was drained by `tick_with_input`.
        assert!(
            app.host.pending_host_intents.is_empty(),
            "intent queue should drain on tick",
        );
    }

    /// Submitting an empty draft is a no-op — no toast, no draft
    /// state change. Prevents "ack blanks" noise in the UI.
    #[test]
    fn location_submitted_empty_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // No prior LocationEdited, so draft is None.
        let _ = app.update(Message::LocationSubmitted);
        assert!(app.host.toast_queue.is_empty());
        assert!(app.location_draft.is_none());
    }

    /// `location_value` returns the draft when present, else mirrors
    /// the view-model's projected location. After a tick lands a
    /// view-model, the display should reflect it (until the user
    /// types again).
    #[test]
    fn location_value_prefers_draft_over_view_model() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        // Populate the view-model cache via a Tick.
        let _ = app.update(Message::Tick);

        // Before any draft: display mirrors view-model.
        let without_draft = app.location_value();
        // Draft takes precedence.
        app.location_draft = Some("typed-but-not-submitted".into());
        assert_eq!(app.location_value(), "typed-but-not-submitted");
        // Clearing draft reverts to view-model source.
        app.location_draft = None;
        assert_eq!(app.location_value(), without_draft);
    }

    /// Ctrl+L is intercepted by the app before runtime translation —
    /// no tick runs, so `last_view_model` stays unpopulated even
    /// though a keyboard event arrived. The focus task returned from
    /// `update` isn't directly observable in a unit test (it drives
    /// iced's internal widget state), so we assert on the tick
    /// side-channel.
    #[test]
    fn ctrl_l_hotkey_bypasses_runtime_tick() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character("l".into()),
            modified_key: iced::keyboard::Key::Character("l".into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        });
        let _task = app.update(Message::IcedEvent(event));
        assert!(
            app.last_view_model.is_none(),
            "Ctrl+L should be consumed by the app; no runtime tick",
        );
    }

    /// A bare 'l' keypress (no ctrl) is a normal key event and
    /// should NOT be caught by the hotkey handler. It flows through
    /// to `iced_events::from_iced_event` like any other key. Since
    /// 'l' isn't in the translation subset, translation returns
    /// None and no tick runs — but the reason is "no translation,"
    /// not "hotkey interception."
    #[test]
    fn bare_l_keypress_is_not_a_hotkey() {
        let event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character("l".into()),
            modified_key: iced::keyboard::Key::Character("l".into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        assert!(!super::is_focus_location_hotkey(&event));
    }

    /// Cursor position cached from iced events survives into the
    /// `FrameHostInput` port-layer reads. Mouse movement via
    /// `iced_events::from_iced_event` also translates into a
    /// `HostEvent::PointerMoved`, but the cached value is what
    /// `HostInputPort::pointer_hover_position` surfaces.
    #[test]
    fn cursor_cache_syncs_from_iced_events() {
        use iced::Point;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.host.cursor_position.is_none(), "starts uncached");
        let _task = app.update(Message::IcedEvent(iced::Event::Mouse(
            iced::mouse::Event::CursorMoved {
                position: Point { x: 12.5, y: 34.5 },
            },
        )));
        assert_eq!(app.host.cursor_position, Some(Point::new(12.5, 34.5)));

        // CursorLeft clears the cache.
        let _task = app.update(Message::IcedEvent(iced::Event::Mouse(
            iced::mouse::Event::CursorLeft,
        )));
        assert!(
            app.host.cursor_position.is_none(),
            "CursorLeft should clear"
        );
    }
}
