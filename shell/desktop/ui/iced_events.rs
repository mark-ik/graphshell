/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced → `HostEvent` translation — M5.3.
//!
//! Mirrors [`crate::shell::desktop::workbench::ux_replay::HostEvent::from_egui_event`]
//! for iced's event types. Populates `FrameHostInput::events` for the
//! iced host so the runtime's tick path sees the same host-neutral
//! vocabulary the egui host already produces.
//!
//! Gated behind the `iced-host` feature so the iced crate stays optional.
//!
//! Key translation is deliberately lossy and matches the subset covered
//! by `HostEvent::to_egui_events` (`Enter`, `Escape`, `Space`, `Backspace`,
//! `Delete`, arrow keys, `A`/`S`/`D`/`W`). Keys outside that set return
//! `None` so the runtime's keyboard phase keeps reading them from the
//! host directly until an explicit key-translation pass lands.

use iced::keyboard::{self, key::Named};
use iced::mouse::{self, ScrollDelta};

use crate::shell::desktop::workbench::ux_replay::{HostEvent, ModifiersState, PointerButton};

/// Translate a single `iced::Event` into a `HostEvent`, returning `None`
/// for events with no host-neutral equivalent (`Window::RedrawRequested`,
/// `CursorEntered/Left`, `Touch::*`, file drop, etc.). Those events
/// remain host-local until a future expansion of the `HostEvent`
/// vocabulary covers them.
pub(crate) fn from_iced_event(event: &iced::Event) -> Option<HostEvent> {
    match event {
        iced::Event::Mouse(mouse_event) => from_iced_mouse_event(mouse_event),
        iced::Event::Keyboard(keyboard_event) => from_iced_keyboard_event(keyboard_event),
        iced::Event::Window(window_event) => from_iced_window_event(window_event),
        iced::Event::Touch(_) => None,
        // iced 0.14 added `InputMethod` for IME events. We don't have a
        // host-neutral IME vocabulary yet; drop them until an explicit
        // IME pass lands in `HostEvent`.
        iced::Event::InputMethod(_) => None,
    }
}

fn from_iced_mouse_event(event: &mouse::Event) -> Option<HostEvent> {
    Some(match event {
        mouse::Event::CursorMoved { position } => HostEvent::PointerMoved {
            x: position.x,
            y: position.y,
        },
        mouse::Event::ButtonPressed(button) => HostEvent::PointerDown {
            x: 0.0,
            y: 0.0,
            button: pointer_button_from_iced(*button),
        },
        mouse::Event::ButtonReleased(button) => HostEvent::PointerUp {
            x: 0.0,
            y: 0.0,
            button: pointer_button_from_iced(*button),
        },
        mouse::Event::WheelScrolled { delta } => {
            let (dx, dy) = match delta {
                ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => (*x, *y),
            };
            HostEvent::Scroll { dx, dy }
        }
        mouse::Event::CursorEntered | mouse::Event::CursorLeft => return None,
    })
}

fn from_iced_keyboard_event(event: &keyboard::Event) -> Option<HostEvent> {
    match event {
        keyboard::Event::KeyPressed {
            key,
            modifiers,
            text,
            ..
        } => keyboard_event_as_host(key, *modifiers, true, text.as_deref()),
        keyboard::Event::KeyReleased { key, modifiers, .. } => {
            keyboard_event_as_host(key, *modifiers, false, None)
        }
        keyboard::Event::ModifiersChanged(_) => None,
    }
}

fn from_iced_window_event(event: &iced::window::Event) -> Option<HostEvent> {
    match event {
        iced::window::Event::Focused => Some(HostEvent::Focus(true)),
        iced::window::Event::Unfocused => Some(HostEvent::Focus(false)),
        iced::window::Event::Resized(size) => Some(HostEvent::WindowResized {
            width: size.width,
            height: size.height,
        }),
        _ => None,
    }
}

fn keyboard_event_as_host(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
    pressed: bool,
    text: Option<&str>,
) -> Option<HostEvent> {
    // When a key press produces text, forward the text channel too —
    // matches egui's parallel `Event::Text` pathway.
    if pressed
        && let Some(text) = text
        && !text.is_empty()
    {
        return Some(HostEvent::Text(text.to_string()));
    }

    let key_name = iced_key_to_host_string(key)?;
    Some(HostEvent::Key {
        key: key_name,
        pressed,
        modifiers: modifiers_from_iced(modifiers),
    })
}

fn iced_key_to_host_string(key: &keyboard::Key) -> Option<String> {
    let name = match key {
        keyboard::Key::Named(named) => match named {
            Named::Enter => "Enter",
            Named::Escape => "Escape",
            Named::Space => "Space",
            Named::Backspace => "Backspace",
            Named::Delete => "Delete",
            Named::ArrowUp => "ArrowUp",
            Named::ArrowDown => "ArrowDown",
            Named::ArrowLeft => "ArrowLeft",
            Named::ArrowRight => "ArrowRight",
            _ => return None,
        },
        keyboard::Key::Character(c) => match c.as_ref() {
            "a" | "A" => "A",
            "s" | "S" => "S",
            "d" | "D" => "D",
            "w" | "W" => "W",
            _ => return None,
        },
        keyboard::Key::Unidentified => return None,
    };
    Some(name.to_string())
}

fn pointer_button_from_iced(button: mouse::Button) -> PointerButton {
    match button {
        mouse::Button::Left => PointerButton::Primary,
        mouse::Button::Right => PointerButton::Secondary,
        mouse::Button::Middle => PointerButton::Middle,
        mouse::Button::Back => PointerButton::Back,
        mouse::Button::Forward => PointerButton::Forward,
        mouse::Button::Other(code) => PointerButton::Other(code),
    }
}

fn modifiers_from_iced(modifiers: keyboard::Modifiers) -> ModifiersState {
    ModifiersState {
        alt: modifiers.alt(),
        ctrl: modifiers.control(),
        shift: modifiers.shift(),
        mac_cmd: modifiers.logo(),
        command: modifiers.command(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;

    #[test]
    fn cursor_moved_translates() {
        let event = iced::Event::Mouse(mouse::Event::CursorMoved {
            position: Point { x: 10.0, y: 20.0 },
        });
        match from_iced_event(&event).expect("should translate") {
            HostEvent::PointerMoved { x, y } => {
                assert_eq!(x, 10.0);
                assert_eq!(y, 20.0);
            }
            other => panic!("expected PointerMoved, got {other:?}"),
        }
    }

    #[test]
    fn button_press_translates() {
        let event = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        match from_iced_event(&event).expect("should translate") {
            HostEvent::PointerDown { button, .. } => {
                assert_eq!(button, PointerButton::Primary);
            }
            other => panic!("expected PointerDown, got {other:?}"),
        }
    }

    #[test]
    fn wheel_scroll_translates_lines_and_pixels() {
        let lines = iced::Event::Mouse(mouse::Event::WheelScrolled {
            delta: ScrollDelta::Lines { x: 1.0, y: -2.0 },
        });
        let pixels = iced::Event::Mouse(mouse::Event::WheelScrolled {
            delta: ScrollDelta::Pixels { x: 3.0, y: -4.0 },
        });
        for (event, want_dx, want_dy) in [(lines, 1.0, -2.0), (pixels, 3.0, -4.0)] {
            match from_iced_event(&event).expect("should translate") {
                HostEvent::Scroll { dx, dy } => {
                    assert_eq!(dx, want_dx);
                    assert_eq!(dy, want_dy);
                }
                other => panic!("expected Scroll, got {other:?}"),
            }
        }
    }

    #[test]
    fn named_key_translates_to_limited_subset() {
        let modifiers = keyboard::Modifiers::empty();
        let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Enter),
            modified_key: keyboard::Key::Named(Named::Enter),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        });
        match from_iced_event(&event).expect("should translate") {
            HostEvent::Key { key, pressed, .. } => {
                assert_eq!(key, "Enter");
                assert!(pressed);
            }
            other => panic!("expected Key, got {other:?}"),
        }
    }

    #[test]
    fn character_outside_subset_returns_none() {
        let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character("z".into()),
            modified_key: keyboard::Key::Character("z".into()),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        // 'z' isn't in the limited key subset; translation returns None.
        assert!(from_iced_event(&event).is_none());
    }

    #[test]
    fn window_focus_translates() {
        assert_eq!(
            from_iced_event(&iced::Event::Window(iced::window::Event::Focused)),
            Some(HostEvent::Focus(true)),
        );
        assert_eq!(
            from_iced_event(&iced::Event::Window(iced::window::Event::Unfocused)),
            Some(HostEvent::Focus(false)),
        );
    }
}
