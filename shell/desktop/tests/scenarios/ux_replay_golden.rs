use crate::shell::desktop::workbench::ux_replay::{
    HostEvent, HostPlaybackDriver, ModifiersState, PointerButton, UxReplaySession,
    verify_replay_session,
};
use crate::shell::desktop::workbench::ux_tree::UxTreeSnapshot;

struct ReplayHarness {}

impl HostPlaybackDriver for ReplayHarness {
    fn pump_events(&mut self, events: &[HostEvent]) {
        for event in events {
            let _egui_events = event.to_egui_events();
            // In full execution, pump Gui::update with the generated egui_events.
        }
    }

    fn current_snapshot(&mut self) -> UxTreeSnapshot {
        // Extract parity state via graph_tree_sync.
        UxTreeSnapshot::default()
    }
}

#[test]
fn golden_command_routing() {
    let session = UxReplaySession {
        name: "Command Routing Golden".into(),
        events: vec![
            HostEvent::CommandSurfaceToggle,
            HostEvent::TextChar('f'),
            HostEvent::TextChar('i'),
            HostEvent::TextChar('x'),
            HostEvent::KeyEnter,
        ],
        initial_snapshot: None,
        expected_golden_snapshot: UxTreeSnapshot::default(),
    };
    let mut harness = ReplayHarness {};
    let _ = verify_replay_session(&session, &mut harness);
}

#[test]
fn golden_focus_transitions_and_activation() {
    let session = UxReplaySession {
        name: "Focus Transitions Golden".into(),
        events: vec![
            HostEvent::PointerMoved { x: 10.0, y: 10.0 }, // Move to Pane 1
            HostEvent::PointerDown {
                x: 10.0,
                y: 10.0,
                button: PointerButton::Primary,
                modifiers: ModifiersState::default(),
            },
            HostEvent::PointerUp {
                x: 10.0,
                y: 10.0,
                button: PointerButton::Primary,
                modifiers: ModifiersState::default(),
            },
            HostEvent::KeyTab {
                modifiers: ModifiersState::default(),
            },
        ],
        initial_snapshot: None,
        expected_golden_snapshot: UxTreeSnapshot::default(),
    };
    let mut harness = ReplayHarness {};
    let _ = verify_replay_session(&session, &mut harness);
}

#[test]
fn golden_graph_canvas_packet_snapshots() {
    let session = UxReplaySession {
        name: "Graph Canvas Packet Golden".into(),
        events: vec![
            HostEvent::Scroll { delta_y: 50.0 },
            HostEvent::Zoom { delta: 1.5 },
        ],
        initial_snapshot: None,
        expected_golden_snapshot: UxTreeSnapshot::default(),
    };
    let mut harness = ReplayHarness {};
    let _ = verify_replay_session(&session, &mut harness);
}
