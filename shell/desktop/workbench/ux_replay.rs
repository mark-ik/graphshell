/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::ux_tree::UxTreeSnapshot;

// HostEvent + PointerButton + ModifiersState moved to
// `graphshell_core::host_event` in M4 slice 8 (2026-04-22).
// Re-exported here so existing import paths resolve unchanged.
pub(crate) use graphshell_core::host_event::{HostEvent, ModifiersState, PointerButton};

/// A captured sequence of actions and the structural expectation it should produce.
/// This matches M0's "same state in -> same runtime outputs out" mandate.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxReplaySession {
    /// The state the workbench was in before any actions were taken.
    pub(crate) initial_snapshot: Option<UxTreeSnapshot>,
    /// The sequence of input pulses.
    pub(crate) events: Vec<HostEvent>,
    /// The exact topological state expected after the last tick updates the GraphTree.
    pub(crate) expected_final_snapshot: Option<UxTreeSnapshot>,
}

pub(crate) fn pointer_button_to_egui(button: PointerButton) -> Option<egui::PointerButton> {
    match button {
        PointerButton::Primary => Some(egui::PointerButton::Primary),
        PointerButton::Secondary => Some(egui::PointerButton::Secondary),
        PointerButton::Middle => Some(egui::PointerButton::Middle),
        PointerButton::Back => Some(egui::PointerButton::Extra1),
        PointerButton::Forward => Some(egui::PointerButton::Extra2),
        PointerButton::Other(_) => None,
    }
}

pub(crate) fn pointer_button_from_egui(button: egui::PointerButton) -> PointerButton {
    match button {
        egui::PointerButton::Primary => PointerButton::Primary,
        egui::PointerButton::Secondary => PointerButton::Secondary,
        egui::PointerButton::Middle => PointerButton::Middle,
        egui::PointerButton::Extra1 => PointerButton::Back,
        egui::PointerButton::Extra2 => PointerButton::Forward,
    }
}

pub(crate) fn modifiers_to_egui(modifiers: ModifiersState) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.alt,
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        mac_cmd: modifiers.mac_cmd,
        command: modifiers.command,
    }
}

pub(crate) fn modifiers_from_egui(modifiers: egui::Modifiers) -> ModifiersState {
    ModifiersState {
        alt: modifiers.alt,
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        mac_cmd: modifiers.mac_cmd,
        command: modifiers.command,
    }
}

/// Translates a live `egui::Event` into a host-neutral `HostEvent`,
/// returning `None` for egui-only events that have no equivalent
/// (`Copy`, `Cut`, `Paste`, `PointerGone`, `AccessKitActionRequest`,
/// etc.). Used to populate `FrameHostInput::events` from the currently
/// running egui host; an iced host will implement the same
/// construction against its own native event stream.
///
/// Key translation is deliberately lossy — `HostEvent::Key::key` uses
/// a short debug-style string (`"Enter"`, `"ArrowUp"`, `"A"`). This
/// mirrors the coverage of `host_event_to_egui_events` and is sufficient for
/// replay + parity tests. Keys outside that set translate to `None`
/// so the runtime's keyboard phase keeps reading them from egui
/// directly until an explicit key-translation pass lands.
pub(crate) fn host_event_from_egui_event(event: &egui::Event) -> Option<HostEvent> {
    Some(match event {
        egui::Event::Text(text) => HostEvent::Text(text.clone()),
        egui::Event::PointerMoved(pos) => HostEvent::PointerMoved { x: pos.x, y: pos.y },
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            ..
        } => {
            let button = pointer_button_from_egui(*button);
            if *pressed {
                HostEvent::PointerDown {
                    x: pos.x,
                    y: pos.y,
                    button,
                }
            } else {
                HostEvent::PointerUp {
                    x: pos.x,
                    y: pos.y,
                    button,
                }
            }
        }
        egui::Event::Zoom(delta) => HostEvent::Zoom { delta: *delta },
        egui::Event::MouseWheel { delta, .. } => HostEvent::Scroll {
            dx: delta.x,
            dy: delta.y,
        },
        egui::Event::WindowFocused(focused) => HostEvent::Focus(*focused),
        egui::Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } => HostEvent::Key {
            key: egui_key_to_host_string(*key)?,
            pressed: *pressed,
            modifiers: modifiers_from_egui(*modifiers),
        },
        _ => return None,
    })
}

/// Translates a host-neutral record playback step into an array of concrete `egui::Event` instances.
/// (Returns a Vec because some synthetic actions may require multiple tick-level egui interactions).
pub(crate) fn host_event_to_egui_events(event: &HostEvent) -> Vec<egui::Event> {
    match event {
        HostEvent::PointerMoved { x, y } => {
            vec![egui::Event::PointerMoved(egui::pos2(*x, *y))]
        }
        HostEvent::PointerDown { x, y, button } => {
            if let Some(btn) = pointer_button_to_egui(*button) {
                vec![egui::Event::PointerButton {
                    pos: egui::pos2(*x, *y),
                    button: btn,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                }]
            } else {
                vec![]
            }
        }
        HostEvent::PointerUp { x, y, button } => {
            if let Some(btn) = pointer_button_to_egui(*button) {
                vec![egui::Event::PointerButton {
                    pos: egui::pos2(*x, *y),
                    button: btn,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                }]
            } else {
                vec![]
            }
        }
        HostEvent::Scroll { dx, dy } => {
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(*dx, *dy),
                modifiers: egui::Modifiers::default(),
                phase: egui::TouchPhase::Move,
            }]
        }
        HostEvent::Zoom { delta } => {
            vec![egui::Event::Zoom(*delta)]
        }
        HostEvent::Text(t) => {
            vec![egui::Event::Text(t.clone())]
        }
        HostEvent::Key {
            key,
            pressed,
            modifiers,
        } => {
            // Simplified key mapping for structural tests to avoid huge switch matching
            let egui_key = match key.as_str() {
                "Enter" => Some(egui::Key::Enter),
                "Escape" => Some(egui::Key::Escape),
                "Space" => Some(egui::Key::Space),
                "Backspace" => Some(egui::Key::Backspace),
                "Delete" => Some(egui::Key::Delete),
                "ArrowUp" => Some(egui::Key::ArrowUp),
                "ArrowDown" => Some(egui::Key::ArrowDown),
                "ArrowLeft" => Some(egui::Key::ArrowLeft),
                "ArrowRight" => Some(egui::Key::ArrowRight),
                "A" | "a" => Some(egui::Key::A),
                "S" | "s" => Some(egui::Key::S),
                "D" | "d" => Some(egui::Key::D),
                "W" | "w" => Some(egui::Key::W),
                // For thorough key mapping, you'd map the entire range of winit keys.
                // This is sufficient for M0 UI parity driving tests.
                _ => None,
            };
            if let Some(k) = egui_key {
                vec![egui::Event::Key {
                    key: k,
                    physical_key: None,
                    pressed: *pressed,
                    repeat: false,
                    modifiers: modifiers_to_egui(*modifiers),
                }]
            } else {
                vec![]
            }
        }
        HostEvent::WindowResized { .. } => {
            vec![]
        }
        HostEvent::Focus(focused) => {
            vec![egui::Event::WindowFocused(*focused)]
        }
        HostEvent::CommandSurface { .. } => {
            // These are injected through context rather than OS pointer inputs.
            vec![]
        }
    }
}

/// Inverse of the limited key-translation done in [`host_event_to_egui_events`].
/// Returns the short stable string keys produced by the to_egui path and
/// `None` for keys outside that subset.
fn egui_key_to_host_string(key: egui::Key) -> Option<String> {
    let name = match key {
        egui::Key::Enter => "Enter",
        egui::Key::Escape => "Escape",
        egui::Key::Space => "Space",
        egui::Key::Backspace => "Backspace",
        egui::Key::Delete => "Delete",
        egui::Key::ArrowUp => "ArrowUp",
        egui::Key::ArrowDown => "ArrowDown",
        egui::Key::ArrowLeft => "ArrowLeft",
        egui::Key::ArrowRight => "ArrowRight",
        egui::Key::A => "A",
        egui::Key::S => "S",
        egui::Key::D => "D",
        egui::Key::W => "W",
        _ => return None,
    };
    Some(name.to_string())
}

/// An abstraction allowing generic host-level tests to feed events into
/// egui (or iced in the future) and measure the resulting structural topology.
pub(crate) trait HostPlaybackDriver {
    /// Feed a batch of events and compute one or more update boundaries.
    fn pump_events(&mut self, events: &[HostEvent]);

    /// Return the current topological snapshot of the app's UI layers.
    fn current_snapshot(&mut self) -> UxTreeSnapshot;
}

/// Execute a replay session against any host driver, verifying that the
/// "same state in -> same runtime outputs out" mandate is preserved.
pub(crate) fn verify_replay_session<D: HostPlaybackDriver>(
    session: &UxReplaySession,
    driver: &mut D,
) -> Result<(), String> {
    if let Some(initial) = &session.initial_snapshot {
        let current = driver.current_snapshot();
        if &current != initial {
            return Err("Initial snapshot parity mismatch before any events were pumped.".into());
        }
    }

    driver.pump_events(&session.events);

    if let Some(expected) = &session.expected_final_snapshot {
        let current = driver.current_snapshot();
        if &current != expected {
            // In a real test, you'd use pretty_assertions or similar to diff
            // the snapshots to report the exact failing tree branch.
            return Err("Final snapshot parity mismatch after pumping events.".into());
        }
    }

    Ok(())
}
