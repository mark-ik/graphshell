/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host-parity tests for the iced host — M5.5.
//!
//! Two levels of parity to care about:
//!
//! 1. **Event translation parity** — the egui and iced translators must
//!    produce the same `HostEvent` sequence when fed semantically
//!    equivalent native events. This is the "inputs agree" level.
//!
//! 2. **Runtime tick parity** — given identical `FrameHostInput`, two
//!    independent `GraphshellRuntime` instances (one driven through
//!    `EguiHostPorts`, one through `IcedHostPorts`) end a tick with the
//!    same observable state. The runtime is host-neutral by construction,
//!    so this mostly pins that down with a regression test.
//!
//! This module is pure tests — all behavior lives in a `#[cfg(test)] mod
//! tests` block gated by the `iced-host` feature.

#[cfg(test)]
mod tests {
    use crate::shell::desktop::ui::egui_host_ports::EguiHostPorts;
    use crate::shell::desktop::ui::frame_model::FrameHostInput;
    use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
    use crate::shell::desktop::ui::iced_events::from_iced_event;
    use crate::shell::desktop::ui::iced_host_ports::IcedHostPorts;
    use crate::shell::desktop::workbench::ux_replay::{HostEvent, PointerButton};

    // -----------------------------------------------------------------------
    // Level 1 — event translation parity.
    // -----------------------------------------------------------------------

    #[test]
    fn cursor_move_translates_equivalently() {
        let egui_event = egui::Event::PointerMoved(egui::pos2(5.0, 7.0));
        let iced_event = iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position: iced::Point { x: 5.0, y: 7.0 },
        });

        let from_egui = HostEvent::from_egui_event(&egui_event).expect("egui translates");
        let from_iced = from_iced_event(&iced_event).expect("iced translates");
        assert_eq!(from_egui, from_iced);
    }

    #[test]
    fn button_press_translates_equivalently() {
        let egui_event = egui::Event::PointerButton {
            pos: egui::pos2(10.0, 20.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        };
        let iced_event = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
            iced::mouse::Button::Left,
        ));

        let from_egui = HostEvent::from_egui_event(&egui_event).expect("egui translates");
        let from_iced = from_iced_event(&iced_event).expect("iced translates");

        // Both should produce a PointerDown with PointerButton::Primary.
        // The iced translation synthesizes (0.0, 0.0) because iced's
        // ButtonPressed event carries no position — the coordinates come
        // from a paired CursorMoved event in the live stream. This is a
        // known asymmetry; parity asserts only the button + event kind.
        match (from_egui, from_iced) {
            (
                HostEvent::PointerDown {
                    button: egui_button,
                    ..
                },
                HostEvent::PointerDown {
                    button: iced_button,
                    ..
                },
            ) => {
                assert_eq!(egui_button, iced_button);
                assert_eq!(egui_button, PointerButton::Primary);
            }
            other => panic!("expected both to be PointerDown, got {other:?}"),
        }
    }

    #[test]
    fn named_key_press_translates_equivalently() {
        let egui_event = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        let iced_event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
        });

        let from_egui = HostEvent::from_egui_event(&egui_event).expect("egui translates");
        let from_iced = from_iced_event(&iced_event).expect("iced translates");
        assert_eq!(from_egui, from_iced);
    }

    #[test]
    fn window_focus_translates_equivalently() {
        let egui_event = egui::Event::WindowFocused(true);
        let iced_event = iced::Event::Window(iced::window::Event::Focused);

        let from_egui = HostEvent::from_egui_event(&egui_event).expect("egui translates");
        let from_iced = from_iced_event(&iced_event).expect("iced translates");
        assert_eq!(from_egui, from_iced);
    }

    // -----------------------------------------------------------------------
    // Level 2 — runtime tick parity.
    // -----------------------------------------------------------------------

    /// Two independently-constructed `GraphshellRuntime`s, one driven
    /// through the egui port bundle and one through the iced port bundle,
    /// agree on the observable graph state after the same tick input.
    ///
    /// This is mostly a regression-pinning test: the runtime is host-neutral
    /// by construction, so tick behavior cannot depend on which port bundle
    /// is supplied unless a port impl mutates runtime state during tick.
    /// Both bundles' currently-wired ports (toast + clipboard for egui,
    /// placeholders for iced) touch no runtime-observable state, so the
    /// post-tick node counts and focus must agree.
    #[test]
    fn runtime_tick_parity_across_host_ports() {
        let input = FrameHostInput::default();

        let mut runtime_egui = GraphshellRuntime::for_testing();
        let mut runtime_iced = GraphshellRuntime::for_testing();

        // egui port bundle needs real toast + clipboard holders.
        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<arboard::Clipboard> = None;
        let mut egui_ports = EguiHostPorts {
            toasts: &mut toasts,
            clipboard: &mut clipboard,
        };
        let mut iced_ports = IcedHostPorts;

        let vm_egui = runtime_egui.tick(&input, &mut egui_ports);
        let vm_iced = runtime_iced.tick(&input, &mut iced_ports);

        // View models agree on their host-neutral shape. We compare the
        // cheapest stable fields; equality of the full model is not
        // guaranteed across runs because of `Instant`-typed focus-ring
        // timestamps.
        assert_eq!(
            vm_egui.active_pane_rects.len(),
            vm_iced.active_pane_rects.len()
        );
        assert_eq!(vm_egui.tree_rows.len(), vm_iced.tree_rows.len());
        assert_eq!(vm_egui.tab_order.len(), vm_iced.tab_order.len());
        assert_eq!(vm_egui.active_pane, vm_iced.active_pane);
        assert_eq!(
            vm_egui.focus.graph_surface_focused,
            vm_iced.focus.graph_surface_focused,
        );

        // Underlying runtime state agrees.
        assert_eq!(
            runtime_egui
                .graph_app
                .domain_graph()
                .nodes()
                .count(),
            runtime_iced
                .graph_app
                .domain_graph()
                .nodes()
                .count(),
        );
    }

    /// Host-neutral `UxTreeSnapshot` parity — both hosts' runtimes produce
    /// identical snapshots via `build_snapshot_host_neutral` after the
    /// same tick. Pins §5.1's invariant: the non-pane portion of the
    /// snapshot is host-independent.
    #[test]
    fn host_neutral_snapshot_parity_across_host_ports() {
        use crate::shell::desktop::workbench::ux_tree::build_snapshot_host_neutral;

        let input = FrameHostInput::default();

        let mut runtime_egui = GraphshellRuntime::for_testing();
        let mut runtime_iced = GraphshellRuntime::for_testing();

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<arboard::Clipboard> = None;
        let mut egui_ports = EguiHostPorts {
            toasts: &mut toasts,
            clipboard: &mut clipboard,
        };
        let mut iced_ports = IcedHostPorts;

        let _ = runtime_egui.tick(&input, &mut egui_ports);
        let _ = runtime_iced.tick(&input, &mut iced_ports);

        let snap_egui = build_snapshot_host_neutral(&runtime_egui.graph_app, 0);
        let snap_iced = build_snapshot_host_neutral(&runtime_iced.graph_app, 0);

        // Build durations zeroed; trace_summary's `build_duration_us` is
        // the only time-varying field, and we passed 0 on both sides.
        assert_eq!(snap_egui, snap_iced);
    }
}
