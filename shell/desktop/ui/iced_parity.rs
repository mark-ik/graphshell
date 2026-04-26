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
    use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
    use crate::shell::desktop::ui::iced_events::from_iced_event;
    use crate::shell::desktop::ui::iced_host_ports::IcedHostPorts;
    use crate::shell::desktop::workbench::ux_replay::{
        HostEvent, PointerButton, host_event_from_egui_event,
    };
    use graphshell_runtime::FrameHostInput;

    // -----------------------------------------------------------------------
    // Level 1 — event translation parity.
    // -----------------------------------------------------------------------

    #[test]
    fn cursor_move_translates_equivalently() {
        let egui_event = egui::Event::PointerMoved(egui::pos2(5.0, 7.0));
        let iced_event = iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position: iced::Point { x: 5.0, y: 7.0 },
        });

        let from_egui = host_event_from_egui_event(&egui_event).expect("egui translates");
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
        let iced_event =
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));

        let from_egui = host_event_from_egui_event(&egui_event).expect("egui translates");
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
            repeat: false,
        });

        let from_egui = host_event_from_egui_event(&egui_event).expect("egui translates");
        let from_iced = from_iced_event(&iced_event).expect("iced translates");
        assert_eq!(from_egui, from_iced);
    }

    #[test]
    fn window_focus_translates_equivalently() {
        let egui_event = egui::Event::WindowFocused(true);
        let iced_event = iced::Event::Window(iced::window::Event::Focused);

        let from_egui = host_event_from_egui_event(&egui_event).expect("egui translates");
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
        let mut pending_webview_a11y_updates = std::collections::HashMap::new();
        let mut pending_accesskit_focus_requests = Vec::new();
        let mut pending_present_requests = Vec::new();
        let mut egui_ports = EguiHostPorts {
            toasts: &mut toasts,
            clipboard: &mut clipboard,
            pending_webview_a11y_updates: &mut pending_webview_a11y_updates,
            pending_accesskit_focus_requests: &mut pending_accesskit_focus_requests,
            ui_render_backend: None,
            pending_present_requests: &mut pending_present_requests,
            ctx: None,
        };
        let mut iced_clipboard: Option<arboard::Clipboard> = None;
        let mut iced_toasts: Vec<graphshell_runtime::ToastSpec> = Vec::new();
        let mut iced_textures = std::collections::HashMap::new();
        let mut iced_pending_presents = Vec::new();
        let mut iced_ports = IcedHostPorts {
            clipboard: &mut iced_clipboard,
            cursor_position: None,
            modifiers: iced::keyboard::Modifiers::empty(),
            toast_queue: &mut iced_toasts,
            texture_cache: &mut iced_textures,
            pending_present_requests: &mut iced_pending_presents,
        };

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
            runtime_egui.graph_app.domain_graph().nodes().count(),
            runtime_iced.graph_app.domain_graph().nodes().count(),
        );
    }

    /// GraphTree-backed snapshot emits real semantic entries.
    ///
    /// Verifies §5.1's done-gate shape: with a populated `GraphTree`,
    /// a GraphTreeWalker-backed build produces a workbench root + the
    /// synthetic container + one NodePane entry per member. The host-
    /// neutral uxtree builder no longer requires `&Tree<TileKind>` to
    /// produce presentation-layer output.
    #[test]
    fn graph_tree_walker_snapshot_emits_pane_entries() {
        use crate::graph::NodeKey;
        use crate::shell::desktop::workbench::ux_tree::{UxNodeRole, build_snapshot_with_walker};
        use crate::shell::desktop::workbench::ux_tree_source::GraphTreeWalker;
        use graph_tree::{Lifecycle, MemberEntry, Provenance, TreeTopology};

        let app = crate::app::GraphBrowserApp::new_for_testing();

        let node = NodeKey::new(0);
        let mut topology = TreeTopology::<NodeKey>::new();
        topology.attach_root(node);
        let tree = graph_tree::GraphTree::<NodeKey>::from_members(
            vec![(
                node,
                MemberEntry::new(Lifecycle::Active, Provenance::Anchor),
            )],
            topology,
            Vec::new(),
            graph_tree::LayoutMode::TreeStyleTabs,
            graph_tree::ProjectionLens::Traversal,
        );

        let walker = GraphTreeWalker::new(&tree);
        let snapshot = build_snapshot_with_walker(&walker, &app, None, 0);

        // Workbench root present regardless of walker.
        assert!(
            snapshot
                .semantic_nodes
                .iter()
                .any(|n| matches!(n.role, UxNodeRole::Workbench)),
            "workbench root missing",
        );

        // Synthetic container from the GraphTree walker — role
        // SplitContainer because our walker synthesizes as Linear.
        assert!(
            snapshot
                .semantic_nodes
                .iter()
                .any(|n| matches!(n.role, UxNodeRole::SplitContainer)),
            "synthetic container missing: semantic roles = {:?}",
            snapshot
                .semantic_nodes
                .iter()
                .map(|n| &n.role)
                .collect::<Vec<_>>(),
        );

        // One NodePane entry for the single member.
        let node_pane_count = snapshot
            .semantic_nodes
            .iter()
            .filter(|n| matches!(n.role, UxNodeRole::NodePane))
            .count();
        assert_eq!(node_pane_count, 1, "expected exactly one NodePane entry");
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
        let mut pending_webview_a11y_updates = std::collections::HashMap::new();
        let mut pending_accesskit_focus_requests = Vec::new();
        let mut pending_present_requests = Vec::new();
        let mut egui_ports = EguiHostPorts {
            toasts: &mut toasts,
            clipboard: &mut clipboard,
            pending_webview_a11y_updates: &mut pending_webview_a11y_updates,
            pending_accesskit_focus_requests: &mut pending_accesskit_focus_requests,
            ui_render_backend: None,
            pending_present_requests: &mut pending_present_requests,
            ctx: None,
        };
        let mut iced_clipboard: Option<arboard::Clipboard> = None;
        let mut iced_toasts: Vec<graphshell_runtime::ToastSpec> = Vec::new();
        let mut iced_textures = std::collections::HashMap::new();
        let mut iced_pending_presents = Vec::new();
        let mut iced_ports = IcedHostPorts {
            clipboard: &mut iced_clipboard,
            cursor_position: None,
            modifiers: iced::keyboard::Modifiers::empty(),
            toast_queue: &mut iced_toasts,
            texture_cache: &mut iced_textures,
            pending_present_requests: &mut iced_pending_presents,
        };

        let _ = runtime_egui.tick(&input, &mut egui_ports);
        let _ = runtime_iced.tick(&input, &mut iced_ports);

        let snap_egui = build_snapshot_host_neutral(
            &runtime_egui.graph_app,
            Some(&runtime_egui.command_surface_telemetry),
            0,
        );
        let snap_iced = build_snapshot_host_neutral(
            &runtime_iced.graph_app,
            Some(&runtime_iced.command_surface_telemetry),
            0,
        );

        // Build durations zeroed; trace_summary's `build_duration_us` is
        // the only time-varying field, and we passed 0 on both sides.
        assert_eq!(snap_egui, snap_iced);
    }

    /// §12.12 (2026-04-24) — first cross-host replay-trace parity test.
    ///
    /// Drives both runtime instances through `runtime.tick(...)` with
    /// IDENTICAL `FrameHostInput.events` (a small UxReplaySession-style
    /// HostEvent sequence) and asserts the resulting `FrameViewModel`
    /// projections match across hosts on the portable scalar fields.
    ///
    /// This is the smallest meaningful cross-host parity exercise — it
    /// validates that the runtime's tick is genuinely host-neutral when
    /// fed real (non-default) input traces. Future slices expand the
    /// trace coverage, add `PartialEq` to the view-model sub-types, and
    /// gate CI on parity divergence.
    #[test]
    fn replay_trace_scalar_parity_across_host_ports() {
        use graphshell_core::host_event::HostEvent;

        // Construct a small replay trace: pointer move, then a
        // primary-button down. Same sequence both runtimes consume.
        let trace_events = vec![
            HostEvent::PointerMoved { x: 32.0, y: 48.0 },
            HostEvent::PointerDown {
                x: 32.0,
                y: 48.0,
                button: PointerButton::Primary,
            },
        ];

        let input = FrameHostInput {
            events: trace_events,
            had_input_events: true,
            ..FrameHostInput::default()
        };

        let mut runtime_egui = GraphshellRuntime::for_testing();
        let mut runtime_iced = GraphshellRuntime::for_testing();

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<arboard::Clipboard> = None;
        let mut pending_webview_a11y_updates = std::collections::HashMap::new();
        let mut pending_accesskit_focus_requests = Vec::new();
        let mut pending_present_requests = Vec::new();
        let mut egui_ports = EguiHostPorts {
            toasts: &mut toasts,
            clipboard: &mut clipboard,
            pending_webview_a11y_updates: &mut pending_webview_a11y_updates,
            pending_accesskit_focus_requests: &mut pending_accesskit_focus_requests,
            ui_render_backend: None,
            pending_present_requests: &mut pending_present_requests,
            ctx: None,
        };
        let mut iced_clipboard: Option<arboard::Clipboard> = None;
        let mut iced_toasts: Vec<graphshell_runtime::ToastSpec> = Vec::new();
        let mut iced_textures = std::collections::HashMap::new();
        let mut iced_pending_presents = Vec::new();
        let mut iced_ports = IcedHostPorts {
            clipboard: &mut iced_clipboard,
            cursor_position: None,
            modifiers: iced::keyboard::Modifiers::empty(),
            toast_queue: &mut iced_toasts,
            texture_cache: &mut iced_textures,
            pending_present_requests: &mut iced_pending_presents,
        };

        let vm_egui = runtime_egui.tick(&input, &mut egui_ports);
        let vm_iced = runtime_iced.tick(&input, &mut iced_ports);

        // Struct-level parity across all host-neutral view-model
        // sub-structs. Each sub-model has `PartialEq` as of 2026-04-24,
        // so any field-level divergence is now caught by a single
        // assertion rather than requiring a scalar allowlist. Divergence
        // here is a kernel regression by construction — the runtime is
        // host-neutral so differing ports can only diverge results if a
        // port impl mutated runtime state during tick.
        assert_eq!(vm_egui.focus, vm_iced.focus, "focus view-model");
        assert_eq!(vm_egui.toolbar, vm_iced.toolbar, "toolbar view-model");
        assert_eq!(vm_egui.omnibar, vm_iced.omnibar, "omnibar view-model");
        assert_eq!(
            vm_egui.graph_search, vm_iced.graph_search,
            "graph_search view-model"
        );
        assert_eq!(
            vm_egui.command_palette, vm_iced.command_palette,
            "command_palette view-model"
        );
        assert_eq!(vm_egui.dialogs, vm_iced.dialogs, "dialogs view-model");
        assert_eq!(vm_egui.settings, vm_iced.settings, "settings view-model");
        assert_eq!(vm_egui.toasts, vm_iced.toasts, "toasts view-model");
        assert_eq!(
            vm_egui.degraded_receipts, vm_iced.degraded_receipts,
            "degraded_receipts view-model"
        );
        assert_eq!(
            vm_egui.captures_in_flight, vm_iced.captures_in_flight,
            "captures_in_flight"
        );
        assert_eq!(vm_egui.active_pane, vm_iced.active_pane, "active_pane");
        assert_eq!(
            vm_egui.is_graph_view, vm_iced.is_graph_view,
            "is_graph_view"
        );
        assert_eq!(
            vm_egui.accessibility, vm_iced.accessibility,
            "accessibility view-model"
        );
    }
}
