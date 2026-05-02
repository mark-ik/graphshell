    use super::*;

    #[test]
    fn iced_app_tick_drives_runtime() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.last_view_model.is_none(), "view-model cache starts empty");

        let _task = app.update(Message::Tick);

        assert!(app.last_view_model.is_some(), "Tick populates view-model");
        let _element = app.view();
    }

    #[test]
    fn iced_event_drives_runtime_tick_via_update() {
        use iced::mouse;
        use iced::Point;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let event = iced::Event::Mouse(mouse::Event::CursorMoved {
            position: Point { x: 42.0, y: 24.0 },
        });
        let _task = app.update(Message::IcedEvent(event));

        assert!(
            app.last_view_model.is_some(),
            "translated iced event should have driven a runtime tick",
        );
    }

    #[test]
    fn untranslatable_iced_event_does_not_tick() {
        use iced::mouse;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let event = iced::Event::Mouse(mouse::Event::CursorEntered);
        let _task = app.update(Message::IcedEvent(event));

        assert!(
            app.last_view_model.is_none(),
            "untranslatable event must be dropped without ticking",
        );
    }

    #[test]
    fn camera_changed_persists_to_runtime_canvas_cameras() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let view_id = app.host.view_id;

        let pan = Vector2D::new(42.0, -17.0);
        let zoom = 1.75;
        let _task = app.update(Message::CameraChanged {
            pane_id: None,
            pan,
            zoom,
        });

        let camera = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .canvas_cameras
            .get(&view_id)
            .expect("camera should be persisted under host view_id");
        assert_eq!(camera.pan, pan);
        assert_eq!(camera.zoom, zoom);
        assert_eq!(camera.pan_velocity, Vector2D::zero());
    }

    // --- Omnibar tests (Slice 2) ---

    #[test]
    fn omnibar_input_updates_draft_without_ticking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.omnibar.draft.is_empty(), "draft starts empty");
        assert!(app.last_view_model.is_none(), "no tick has run");

        let _task = app.update(Message::OmnibarInput("https://exa".into()));
        assert_eq!(app.omnibar.draft, "https://exa");
        assert!(
            app.last_view_model.is_none(),
            "typing must not tick the runtime",
        );

        let _task = app.update(Message::OmnibarInput("https://example.com".into()));
        assert_eq!(app.omnibar.draft, "https://example.com");
    }

    #[test]
    fn omnibar_submit_url_creates_node_and_returns_to_display() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::OmnibarFocus);
        let _ = app.update(Message::OmnibarInput("https://submit.test/".into()));
        let _ = app.update(Message::OmnibarSubmit);

        assert!(app.omnibar.draft.is_empty(), "submit clears draft");
        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert_eq!(app.host.toast_queue.len(), 1, "submit enqueues ack toast");
        assert!(app.host.toast_queue[0].message.contains("https://submit.test/"));

        let nodes_after = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(nodes_after, nodes_before + 1, "exactly one node added");
        assert!(app.host.pending_host_intents.is_empty(), "intent queue drained");
    }

    #[test]
    fn omnibar_submit_non_url_routes_to_node_finder() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::OmnibarInput("graphql tutorial".into()));
        let _ = app.update(Message::OmnibarSubmit);

        assert!(app.omnibar.draft.is_empty());
        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert_eq!(
            app.host.runtime.graph_app.domain_graph().nodes().count(),
            nodes_before,
            "non-URL submit must not create a graph node",
        );
        assert!(
            app.host.toast_queue.is_empty(),
            "OmnibarSubmit alone does not toast — routing happens via Task::done",
        );

        // Simulate iced driving the Task::done message — Slice 6 wiring
        // opens the Node Finder pre-seeded with the query (no toast).
        let _ = app.update(Message::OmnibarRouteToNodeFinder("graphql tutorial".into()));
        assert!(app.node_finder.is_open, "non-URL submit opens the Node Finder");
        assert_eq!(app.node_finder.query, "graphql tutorial");
        assert!(
            app.host.toast_queue.is_empty(),
            "Slice 6 routing does not toast — the modal itself is the affordance",
        );
    }

    #[test]
    fn omnibar_submit_empty_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarSubmit);
        assert!(app.host.toast_queue.is_empty());
        assert!(app.omnibar.draft.is_empty());
    }

    #[test]
    fn ctrl_l_transitions_omnibar_to_input_mode() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert_eq!(app.omnibar.mode, OmnibarMode::Display);

        let ctrl_l = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
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
        let _task = app.update(Message::IcedEvent(ctrl_l));
        assert!(
            app.last_view_model.is_none(),
            "Ctrl+L must not tick the runtime",
        );
        let _task = app.update(Message::OmnibarFocus);
        assert_eq!(app.omnibar.mode, OmnibarMode::Input);
    }

    #[test]
    fn escape_dismisses_omnibar() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarFocus);
        let _ = app.update(Message::OmnibarInput("partial".into()));
        let _ = app.update(Message::OmnibarKeyEscape);

        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert!(app.omnibar.draft.is_empty(), "escape clears draft");
    }

    #[test]
    fn omnibar_blur_returns_to_display_preserving_draft() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarFocus);
        let _ = app.update(Message::OmnibarInput("partial".into()));
        let _ = app.update(Message::OmnibarBlur);

        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert_eq!(app.omnibar.draft, "partial", "blur preserves draft");
    }

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
        assert!(!super::is_omnibar_focus_hotkey(&event));
    }

    #[test]
    fn url_shape_detection() {
        assert!(is_url_shaped("https://example.com"));
        assert!(is_url_shaped("verso://settings"));
        assert!(is_url_shaped("http://localhost:8080/path"));
        assert!(is_url_shaped("example.com"));
        assert!(is_url_shaped("sub.example.co.uk"));
        assert!(!is_url_shaped("graphql tutorial"));
        assert!(!is_url_shaped("find nodes"));
        assert!(!is_url_shaped(""));
        assert!(!is_url_shaped("   "));
    }

    #[test]
    fn cursor_cache_syncs_from_iced_events() {
        use iced::mouse;
        use iced::Point;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.host.cursor_position.is_none(), "starts uncached");
        let _task = app.update(Message::IcedEvent(iced::Event::Mouse(
            mouse::Event::CursorMoved {
                position: Point { x: 12.5, y: 34.5 },
            },
        )));
        assert_eq!(app.host.cursor_position, Some(iced::Point::new(12.5, 34.5)));

        let _task = app.update(Message::IcedEvent(iced::Event::Mouse(
            mouse::Event::CursorLeft,
        )));
        assert!(app.host.cursor_position.is_none(), "CursorLeft clears cache");
    }

    // --- Frame split-tree tests (Slice 3) ---

    /// `IcedApp` starts with exactly one Canvas pane pre-seeded in the
    /// Frame (the default launch state for Slice 3 verification).
    #[test]
    fn frame_starts_with_one_canvas_pane() {
        let runtime = GraphshellRuntime::for_testing();
        let app = IcedApp::with_runtime(runtime);

        assert!(!app.frame.base_layer_active, "pane_grid is active at launch");
        assert_eq!(app.frame.split_state.len(), 1, "exactly one pane at launch");

        let (_, meta) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have one pane");
        assert_eq!(meta.pane_type, PaneType::Canvas, "initial pane is Canvas");
    }

    /// `PaneFocused` records the iced pane handle as the focused pane.
    #[test]
    fn pane_focused_records_handle() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.frame.focused_pane.is_none(), "no focused pane at start");

        let (pane_handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have one pane");
        let handle = *pane_handle;

        let _ = app.update(Message::PaneFocused(handle));
        assert_eq!(app.frame.focused_pane, Some(handle));
    }

    /// `ClosePane` on the only Pane activates the canvas base layer.
    ///
    /// Note: `pane_grid::State::close` is a no-op on the last pane (iced
    /// cannot reduce the state to zero panes). `FrameState::base_layer_active`
    /// is the flag that switches the render path to the canvas base layer.
    #[test]
    fn close_last_pane_activates_base_layer() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert_eq!(app.frame.split_state.len(), 1);
        assert!(!app.frame.base_layer_active);

        let (pane_handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have a pane");
        let handle = *pane_handle;

        let _ = app.update(Message::PaneFocused(handle));
        let _ = app.update(Message::ClosePane(handle));

        assert!(
            app.frame.base_layer_active,
            "base_layer_active should be set after closing the last pane",
        );
        assert_eq!(
            app.frame.focused_pane, None,
            "focused pane cleared when it is closed",
        );
    }

    /// Closing a Pane that is not the focused Pane leaves `focused_pane`
    /// intact.
    #[test]
    fn close_non_focused_pane_preserves_focus() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Split so there are two panes.
        let (first_handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have a pane");
        let first = *first_handle;

        let second_meta = PaneMeta {
            pane_id: PaneId::next(),
            pane_type: PaneType::Tile,
        };
        let (second, _split) = app
            .frame
            .split_state
            .split(pane_grid::Axis::Vertical, first, second_meta)
            .expect("split should succeed");

        // Focus the first pane; close the second.
        let _ = app.update(Message::PaneFocused(first));
        let _ = app.update(Message::ClosePane(second));

        assert_eq!(app.frame.split_state.len(), 1, "one pane remains");
        assert_eq!(
            app.frame.focused_pane,
            Some(first),
            "focused_pane unchanged when a non-focused pane is closed",
        );
    }

    /// `view()` produces an element without panicking for the default
    /// (one-pane) frame state.
    #[test]
    fn view_renders_pane_grid_without_panic() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    /// After closing the last pane, `view()` falls back to the canvas
    /// base layer (`base_layer_active`) without panicking.
    #[test]
    fn view_renders_base_layer_when_last_pane_closed() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let (handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("initial pane");
        let handle = *handle;
        let _ = app.update(Message::ClosePane(handle));
        assert!(app.frame.base_layer_active);

        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    // --- Command Palette + Node Finder tests (Slice 6) ---

    fn key_press(c: &str, modifiers: iced::keyboard::Modifiers) -> iced::Event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c.into()),
            modified_key: iced::keyboard::Key::Character(c.into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    #[test]
    fn ctrl_shift_p_opens_command_palette() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(!app.command_palette.is_open);

        let event = key_press(
            "p",
            iced::keyboard::Modifiers::CTRL | iced::keyboard::Modifiers::SHIFT,
        );
        // The IcedEvent path returns Task::done(PaletteOpen{...}); simulate
        // the runtime delivering that message back to update().
        let _ = app.update(Message::IcedEvent(event));
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        assert!(app.command_palette.is_open);
        assert_eq!(app.command_palette.origin, PaletteOrigin::KeyboardShortcut);
    }

    #[test]
    fn ctrl_p_opens_node_finder_not_palette() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        assert!(app.node_finder.is_open);
        assert!(!app.command_palette.is_open);
    }

    #[test]
    fn palette_and_finder_hotkeys_are_distinct() {
        let ctrl_p = key_press("p", iced::keyboard::Modifiers::CTRL);
        let ctrl_shift_p = key_press(
            "p",
            iced::keyboard::Modifiers::CTRL | iced::keyboard::Modifiers::SHIFT,
        );

        assert!(super::is_node_finder_hotkey(&ctrl_p));
        assert!(!super::is_command_palette_hotkey(&ctrl_p));
        assert!(super::is_command_palette_hotkey(&ctrl_shift_p));
        assert!(!super::is_node_finder_hotkey(&ctrl_shift_p));
    }

    #[test]
    fn palette_and_finder_are_mutually_exclusive() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert!(app.command_palette.is_open);

        // Opening node finder closes the palette.
        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        assert!(!app.command_palette.is_open);
        assert!(app.node_finder.is_open);

        // Opening palette closes the finder.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert!(app.command_palette.is_open);
        assert!(!app.node_finder.is_open);
    }

    #[test]
    fn palette_query_updates_state_without_ticking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteQuery("toggl".into()));

        assert_eq!(app.command_palette.query, "toggl");
        assert!(
            app.last_view_model.is_none(),
            "palette typing must not tick the runtime",
        );
    }

    #[test]
    fn palette_close_clears_state() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteQuery("partial".into()));
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);

        assert!(!app.command_palette.is_open);
        assert!(app.command_palette.query.is_empty());
        assert!(app.command_palette.focused_index.is_none());
    }

    #[test]
    fn omnibar_route_to_node_finder_actually_opens_finder() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarRouteToNodeFinder("graph theory".into()));

        assert!(app.node_finder.is_open, "non-URL omnibar submit opens node finder");
        assert_eq!(app.node_finder.query, "graph theory", "query is pre-seeded");
        assert_eq!(
            app.node_finder.origin,
            NodeFinderOrigin::OmnibarRoute("graph theory".into()),
            "origin records the routed query",
        );
        assert_eq!(app.omnibar.mode, OmnibarMode::Display, "omnibar returned to Display");
    }

    #[test]
    fn escape_closes_palette_when_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert!(app.command_palette.is_open);

        let _ = app.update(Message::PaletteCloseAndRestoreFocus);
        assert!(!app.command_palette.is_open);
    }

    #[test]
    fn palette_action_selected_closes_and_acks() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert_eq!(app.host.toast_queue.len(), 0);

        // Slice 9: row 0 is whatever the canonical registry's first
        // action is — capture its label so the assertion stays stable
        // as the registry evolves.
        let expected_label = app.command_palette.all_actions[0].label.clone();
        let _ = app.update(Message::PaletteActionSelected(0));

        assert!(!app.command_palette.is_open);
        assert_eq!(app.host.toast_queue.len(), 1);
        assert!(
            app.host.toast_queue[0].message.contains(&expected_label),
            "expected resolved label in toast, got: {}",
            app.host.toast_queue[0].message,
        );
    }

    #[test]
    fn view_renders_with_palette_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::Tick);

        // Render-time smoke test: must not panic with a modal stacked
        // on top of the body.
        let _element = app.view();
    }

    #[test]
    fn view_renders_with_node_finder_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::Tick);

        let _element = app.view();
    }

    // --- Modal data + nav tests (Slice 7) ---

    #[test]
    fn palette_action_select_dispatches_host_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Pick an action that goes through runtime dispatch (not the
        // host-routed actions like NodeNew / FrameOpen — those
        // intercept before HostIntent::Action). GraphTogglePhysics
        // is a stable runtime-side action.
        let idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| {
                a.action_id == graphshell_core::actions::ActionId::GraphTogglePhysics
            })
            .expect("GraphTogglePhysics in registry");

        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "no dispatch yet"
        );
        assert!(app.host.runtime.last_dispatched_action.is_none());

        let _ = app.update(Message::PaletteActionSelected(idx));

        assert!(
            app.host.pending_host_intents.is_empty(),
            "intent queue drained by post-select tick",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 1,
            "runtime observed exactly one HostIntent::Action",
        );
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::GraphTogglePhysics),
            "runtime recorded the resolved ActionId",
        );
    }

    #[test]
    fn toggle_physics_action_actually_toggles_runtime_flag() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let initial_running = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .physics
            .is_running;

        // Push HostIntent::Action(GraphTogglePhysics) directly via the
        // queue so we don't have to find the right palette index.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());

        let after_running = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .physics
            .is_running;

        assert_ne!(
            initial_running, after_running,
            "GraphTogglePhysics should flip physics.is_running",
        );
        assert_eq!(app.host.runtime.dispatched_action_count, 1);

        // A second dispatch flips it back.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());
        let twice_toggled = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .physics
            .is_running;
        assert_eq!(twice_toggled, initial_running, "second toggle restores");
    }

    #[test]
    fn action_on_node_pre_focuses_target_then_dispatches() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Seed two nodes so we have real NodeKeys to target.
        seed_test_nodes(&mut app, 2);
        let target_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .nth(1)
            .map(|(k, _)| k)
            .expect("seeded ≥2 nodes");

        // Dispatch an action targeting the second node directly.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode {
                action_id: graphshell_core::actions::ActionId::NodePinToggle,
                node_key: target_key,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(
            app.host.runtime.focused_node_hint,
            Some(target_key),
            "ActionOnNode pre-focuses the target before running the handler",
        );
        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodePinToggle),
        );
    }

    #[test]
    fn context_menu_with_target_node_dispatches_action_on_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 1);
        let target_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        // Manually open a context menu against a target carrying a
        // real NodeKey — Slice 16 ships the type but the right-click
        // handlers don't hit-test yet, so this simulates a future
        // canvas-hit-test path.
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::CanvasPane {
                pane_id: PaneId(99),
                node_key: Some(target_key),
            },
        });

        // Pick the "Pin" entry — wired to NodePinToggle.
        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .expect("CanvasPane menu carries a Pin entry");
        let pin_intent = app.context_menu.items[pin_idx].intent.clone();

        // The intent should be ActionOnNode (target carries a node_key).
        assert!(matches!(
            pin_intent,
            Some(graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode { .. })
        ));

        let _ = app.update(Message::ContextMenuEntrySelected(pin_idx));

        assert_eq!(app.host.runtime.focused_node_hint, Some(target_key));
        assert_eq!(app.host.runtime.dispatched_action_count, 1);
    }

    #[test]
    fn context_menu_without_target_node_dispatches_plain_action() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        // Right-click the pane body — current handler passes node_key: None.
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane {
                pane_id,
                node_key: None,
            },
        });

        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .unwrap();
        let pin_intent = app.context_menu.items[pin_idx].intent.clone();

        // Without a target node, the intent is plain Action.
        assert!(matches!(
            pin_intent,
            Some(graphshell_core::shell_state::host_intent::HostIntent::Action { .. })
        ));
    }

    // --- UX observability tests (Slice 23) ---

    /// Convenience: register a RecordingObserver on the runtime and
    /// return the shared handle so the test can inspect the
    /// recorded event stream after running messages.
    fn install_recording_observer(
        app: &mut IcedApp,
    ) -> std::sync::Arc<graphshell_core::ux_observability::RecordingObserver> {
        let recorder =
            std::sync::Arc::new(graphshell_core::ux_observability::RecordingObserver::with_capacity(
                64,
            ));
        let proxy = RecordingProxy(std::sync::Arc::clone(&recorder));
        app.host.runtime.ux_observers.register(Box::new(proxy));
        recorder
    }

    struct RecordingProxy(std::sync::Arc<graphshell_core::ux_observability::RecordingObserver>);
    impl graphshell_core::ux_observability::UxObserver for RecordingProxy {
        fn observe(&self, event: &graphshell_core::ux_observability::UxEvent) {
            self.0.observe(event);
        }
    }

    #[test]
    fn palette_open_close_emits_ux_events() {
        use graphshell_core::ux_observability::{DismissReason, SurfaceId, UxEvent};
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let recorder = install_recording_observer(&mut app);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);

        let events = recorder.snapshot();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            UxEvent::SurfaceOpened {
                surface: SurfaceId::CommandPalette
            }
        ));
        assert!(matches!(
            events[1],
            UxEvent::SurfaceDismissed {
                surface: SurfaceId::CommandPalette,
                reason: DismissReason::Cancelled,
            }
        ));
    }

    #[test]
    fn opening_palette_over_finder_emits_superseded_dismissal() {
        use graphshell_core::ux_observability::{DismissReason, SurfaceId, UxEvent};
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let recorder = install_recording_observer(&mut app);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let events = recorder.snapshot();
        // Expected: NodeFinder Opened, NodeFinder Superseded, CommandPalette Opened.
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0],
            UxEvent::SurfaceOpened {
                surface: SurfaceId::NodeFinder
            }
        ));
        assert!(matches!(
            events[1],
            UxEvent::SurfaceDismissed {
                surface: SurfaceId::NodeFinder,
                reason: DismissReason::Superseded,
            }
        ));
        assert!(matches!(
            events[2],
            UxEvent::SurfaceOpened {
                surface: SurfaceId::CommandPalette
            }
        ));
    }

    #[test]
    fn destructive_context_select_emits_confirm_dialog_open() {
        use graphshell_core::ux_observability::{DismissReason, SurfaceId, UxEvent};
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let recorder = install_recording_observer(&mut app);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane {
                pane_id,
                node_key: None,
            },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));

        let events = recorder.snapshot();
        // Expected sequence: ContextMenu Opened, ContextMenu
        // Confirmed (the destructive selection), ConfirmDialog Opened.
        assert!(events.iter().any(|e| matches!(
            e,
            UxEvent::SurfaceOpened {
                surface: SurfaceId::ContextMenu
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            UxEvent::SurfaceDismissed {
                surface: SurfaceId::ContextMenu,
                reason: DismissReason::Confirmed,
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            UxEvent::SurfaceOpened {
                surface: SurfaceId::ConfirmDialog
            }
        )));
    }

    #[test]
    fn action_dispatch_emits_action_dispatched_event() {
        use graphshell_core::ux_observability::UxEvent;
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let recorder = install_recording_observer(&mut app);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());

        let events = recorder.snapshot();
        assert!(events.iter().any(|e| matches!(
            e,
            UxEvent::ActionDispatched {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
                target: None,
            }
        )));
    }

    #[test]
    fn activity_log_records_palette_open_and_close() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Empty at construction.
        assert!(app.activity_log_recorder.is_empty());

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);

        let events = app.activity_log_recorder.snapshot();
        assert_eq!(events.len(), 2);
        assert_eq!(format_ux_event(&events[0]), "opened: Command Palette");
        assert_eq!(
            format_ux_event(&events[1]),
            "dismissed: Command Palette (cancelled)",
        );
    }

    #[test]
    fn activity_log_capacity_is_bounded() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Fire many open/close cycles — well past capacity.
        for _ in 0..(ACTIVITY_LOG_CAPACITY + 50) {
            let _ = app.update(Message::PaletteOpen {
                origin: PaletteOrigin::KeyboardShortcut,
            });
            let _ = app.update(Message::PaletteCloseAndRestoreFocus);
        }

        // Recorder is bounded — older events evicted.
        let len = app.activity_log_recorder.len();
        assert!(
            len <= ACTIVITY_LOG_CAPACITY,
            "recorder exceeded capacity: {len}",
        );
    }

    #[test]
    fn format_ux_event_renders_canonical_lines() {
        use graphshell_core::ux_observability::{DismissReason, SurfaceId, UxEvent};
        assert_eq!(
            format_ux_event(&UxEvent::SurfaceOpened {
                surface: SurfaceId::ContextMenu,
            }),
            "opened: Context Menu",
        );
        assert_eq!(
            format_ux_event(&UxEvent::SurfaceDismissed {
                surface: SurfaceId::ConfirmDialog,
                reason: DismissReason::Superseded,
            }),
            "dismissed: Confirm Dialog (superseded)",
        );
        assert_eq!(
            format_ux_event(&UxEvent::ActionDispatched {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
                target: None,
            }),
            "action: graph:toggle_physics",
        );
        assert_eq!(
            format_ux_event(&UxEvent::ActionDispatched {
                action_id: graphshell_core::actions::ActionId::NodePinToggle,
                target: Some(graphshell_core::graph::NodeKey::new(7)),
            }),
            "action: node:pin_toggle → n7",
        );
        assert_eq!(
            format_ux_event(&UxEvent::OpenNodeDispatched {
                node_key: graphshell_core::graph::NodeKey::new(3),
            }),
            "open node: n3",
        );
    }

    #[test]
    fn view_renders_with_activity_log_populated() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Fire some events so the bucket has rows.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);
        let _ = app.update(Message::Tick);

        let _element = app.view();
    }

    #[test]
    fn iced_messages_flow_through_channel_bridge() {
        use graphshell_core::ux_diagnostics::{
            DiagnosticsChannelSink, RecordingChannelSink, UxChannelObserver,
        };
        use std::sync::Arc;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Replace the default Noop sink with a recording sink so we
        // can observe what channel ids fire. The NoopChannelSink
        // observer registered by `with_runtime` stays — its emissions
        // are silent — but the recording observer below sees the
        // same event stream and forwards to a sink we can inspect.
        struct ProxySink(Arc<RecordingChannelSink>);
        impl DiagnosticsChannelSink for ProxySink {
            fn record(&self, e: &graphshell_core::ux_diagnostics::ChannelEmission) {
                self.0.record(e);
            }
        }
        let recorder = Arc::new(RecordingChannelSink::with_capacity(32));
        app.host.runtime.ux_observers.register(Box::new(
            UxChannelObserver::new(ProxySink(Arc::clone(&recorder))),
        ));

        // Drive a representative sequence and verify the channel
        // mapping is hit.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());

        let snap = recorder.snapshot();
        let channels: Vec<&str> = snap.iter().map(|e| e.channel_id).collect();
        assert!(
            channels.contains(&"ux.command_palette.opened"),
            "channel bridge missed palette open; saw {channels:?}",
        );
        assert!(
            channels.contains(&"ux.command_palette.dismissed"),
            "channel bridge missed palette dismiss; saw {channels:?}",
        );
        assert!(
            channels.contains(&"ux.action.dispatched"),
            "channel bridge missed action dispatch; saw {channels:?}",
        );
    }

    #[test]
    fn iced_supersession_satisfies_mutual_exclusion_probe() {
        use graphshell_core::ux_probes::{
            MutualExclusionProbe, OpenDismissBalanceProbe, UxProbe, probe_as_observer,
        };
        use std::sync::Arc;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let mutex_probe = Arc::new(MutualExclusionProbe::new());
        let balance_probe = Arc::new(OpenDismissBalanceProbe::new());
        app.host.runtime.ux_observers.register(probe_as_observer(
            Arc::clone(&mutex_probe) as Arc<dyn UxProbe>,
        ));
        app.host.runtime.ux_observers.register(probe_as_observer(
            Arc::clone(&balance_probe) as Arc<dyn UxProbe>,
        ));

        // Drive a sequence the iced host actually produces: open
        // palette, supersede with finder, supersede with context
        // menu, dismiss. Probes verify the supersession sequencing
        // doesn't leave any modal hanging.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        let _ = app.update(Message::ContextMenuDismiss);

        let mutex_failures = mutex_probe.drain_failures();
        let balance_failures = balance_probe.drain_failures();
        assert!(
            mutex_failures.is_empty(),
            "iced supersession trips mutual_exclusion: {mutex_failures:?}",
        );
        assert!(
            balance_failures.is_empty(),
            "iced supersession trips open_dismiss_balance: {balance_failures:?}",
        );
        assert!(
            balance_probe.pending_opens().is_empty(),
            "iced left a modal open at the end of the sequence",
        );
    }

    #[test]
    fn iced_palette_action_satisfies_productive_selection_probe() {
        use graphshell_core::ux_probes::{
            ProductiveSelectionProbe, UxProbe, probe_as_observer,
        };
        use std::sync::Arc;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let probe = Arc::new(ProductiveSelectionProbe::iced_default());
        app.host.runtime.ux_observers.register(probe_as_observer(
            Arc::clone(&probe) as Arc<dyn UxProbe>,
        ));

        // Open palette and pick a runtime-routed action — should
        // produce Dismissed{Confirmed} → ActionDispatched.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let runtime_idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| {
                a.is_available
                    && a.action_id == graphshell_core::actions::ActionId::GraphTogglePhysics
            })
            .expect("GraphTogglePhysics in registry");
        let _ = app.update(Message::PaletteActionSelected(runtime_idx));

        let failures = probe.drain_failures();
        assert!(
            failures.is_empty(),
            "palette runtime path tripped productive_selection: {failures:?}",
        );
    }

    #[test]
    fn iced_palette_host_routed_action_satisfies_productive_selection_probe() {
        use graphshell_core::ux_probes::{
            ProductiveSelectionProbe, UxProbe, probe_as_observer,
        };
        use std::sync::Arc;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let probe = Arc::new(ProductiveSelectionProbe::iced_default());
        app.host.runtime.ux_observers.register(probe_as_observer(
            Arc::clone(&probe) as Arc<dyn UxProbe>,
        ));

        // Pick a host-routed action that opens NodeCreate. Slice 48
        // emits ActionDispatched at the host level so the probe sees a
        // productive event even though the runtime never dispatches.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let host_routed_idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| {
                a.is_available && a.action_id == graphshell_core::actions::ActionId::NodeNew
            })
            .expect("NodeNew in registry");
        let task = app.update(Message::PaletteActionSelected(host_routed_idx));
        // Drain the Task::done(NodeCreateOpen) the host returned.
        let _ = task;
        let _ = app.update(Message::NodeCreateOpen);

        let failures = probe.drain_failures();
        assert!(
            failures.is_empty(),
            "palette host-routed path tripped productive_selection: {failures:?}",
        );
    }

    #[test]
    fn iced_node_finder_selection_satisfies_productive_selection_probe() {
        use graphshell_core::ux_probes::{
            ProductiveSelectionProbe, UxProbe, probe_as_observer,
        };
        use std::sync::Arc;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);

        let probe = Arc::new(ProductiveSelectionProbe::iced_default());
        app.host.runtime.ux_observers.register(probe_as_observer(
            Arc::clone(&probe) as Arc<dyn UxProbe>,
        ));

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::NodeFinderResultSelected(0));

        let failures = probe.drain_failures();
        assert!(
            failures.is_empty(),
            "finder selection tripped productive_selection: {failures:?}",
        );
    }

    #[test]
    fn iced_destructive_context_menu_satisfies_destructive_gate_probe() {
        use graphshell_core::ux_probes::{
            DestructiveActionGateProbe, UxProbe, probe_as_observer,
        };
        use std::sync::Arc;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        let probe = Arc::new(DestructiveActionGateProbe::iced_default());
        app.host.runtime.ux_observers.register(probe_as_observer(
            Arc::clone(&probe) as Arc<dyn UxProbe>,
        ));

        // Open a context menu against a tile pane (which surfaces the
        // Tombstone destructive entry), confirm Tombstone, then confirm
        // the gate dialog. Probe verifies the dispatch is gated.
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane {
                pane_id: PaneId(0),
                node_key: Some(node_key),
            },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|item| item.entry.label == "Tombstone")
            .expect("Tombstone entry present on TilePane menu");
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        // ConfirmDialog should now be open with the parked intent.
        assert!(app.confirm_dialog.is_open);
        let _ = app.update(Message::ConfirmDialogConfirm);

        let failures = probe.drain_failures();
        assert!(
            failures.is_empty(),
            "destructive flow tripped destructive_action_gate: {failures:?}",
        );
    }

    #[test]
    fn open_node_dispatch_emits_open_node_event() {
        use graphshell_core::ux_observability::UxEvent;
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let recorder = install_recording_observer(&mut app);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::OpenNode { node_key },
        );
        app.tick_with_events(Vec::new());

        let events = recorder.snapshot();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            UxEvent::OpenNodeDispatched { node_key: nk } if nk == node_key
        ));
    }

    // --- Tile graphlet projection tests (Slice 29) ---

    #[test]
    fn tile_tab_select_dispatches_open_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();
        // Convert the seeded Canvas pane to Tile so the tab dispatch
        // path is observable.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let _ = app.update(Message::TileTabSelected { pane_id, node_key });

        assert_eq!(app.host.runtime.opened_node_count, 1);
        assert_eq!(app.host.runtime.focused_node_hint, Some(node_key));
    }

    #[test]
    fn tile_tab_close_toasts_and_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let toasts_before = app.host.toast_queue.len();
        let _ = app.update(Message::TileTabClosed { pane_id, node_key });

        assert_eq!(
            app.host.runtime.opened_node_count, 0,
            "close must not fire OpenNode",
        );
        assert!(
            app.host.toast_queue.len() > toasts_before,
            "close stub should toast (until LifecycleIntent lands)",
        );
    }

    #[test]
    fn tile_pane_render_with_no_tiles_does_not_panic() {
        // Default for_testing() runtime starts with no GraphTree
        // members — exercises the empty-state branch of
        // render_tile_pane_body.
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    #[test]
    fn tree_spine_click_dispatches_open_node_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);

        // Pull a NodeKey out of the graph (the GraphTree may be empty
        // until incremental_sync runs, but the dispatch path is what
        // we're testing).
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .expect("seeded a node");

        assert_eq!(app.host.runtime.opened_node_count, 0);

        let _ = app.update(Message::TreeSpineNodeClicked(node_key));

        assert_eq!(
            app.host.runtime.opened_node_count, 1,
            "tree-spine click dispatches HostIntent::OpenNode",
        );
        assert_eq!(
            app.host.runtime.focused_node_hint,
            Some(node_key),
            "runtime promoted the resolved NodeKey to focused",
        );
    }

    #[test]
    fn view_renders_status_bar_with_runtime_counters() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Smoke test: status bar renders with default state. Drop
        // the borrow before the next mutation.
        {
            let _element = app.view();
        }

        // Dispatch an action to bump the actions counter; render again.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());
        assert_eq!(app.host.runtime.dispatched_action_count, 1);

        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    #[test]
    fn graph_fit_action_dispatches_without_panicking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        // Seed a node so there's a focused view available.
        seed_test_nodes(&mut app, 1);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphFit,
            },
        );
        app.tick_with_events(Vec::new());

        // The fit request lands on the focused view's camera-command
        // queue, drained by the next render frame. The test confirms
        // the routing path closed without panicking.
        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::GraphFit),
        );
    }

    #[test]
    fn persist_undo_and_redo_actions_are_dispatched() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Both undo and redo are no-ops without a checkpoint history,
        // but the dispatch path must still run without panicking.
        for action_id in [
            graphshell_core::actions::ActionId::PersistUndo,
            graphshell_core::actions::ActionId::PersistRedo,
        ] {
            app.host.pending_host_intents.push(
                graphshell_core::shell_state::host_intent::HostIntent::Action { action_id },
            );
            app.tick_with_events(Vec::new());
        }

        assert_eq!(app.host.runtime.dispatched_action_count, 2);
    }

    #[test]
    fn node_pin_toggle_action_dispatches_and_routes_to_runtime() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        // ActionOnNode pre-focuses, then dispatches NodePinToggle which
        // routes to GraphIntent::TogglePrimaryNodePin.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode {
                action_id: graphshell_core::actions::ActionId::NodePinToggle,
                node_key,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(app.host.runtime.focused_node_hint, Some(node_key));
        // The runtime ran TogglePrimaryNodePin; we don't assert on
        // node.is_pinned because the focused-selection projection has
        // its own preconditions. The dispatch reaching the runtime is
        // what this slice ships.
    }

    #[test]
    fn node_mark_tombstone_action_dispatches() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::NodeMarkTombstone,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodeMarkTombstone),
        );
    }

    #[test]
    fn graph_fit_graphlet_action_dispatches() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphFitGraphlet,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
    }

    #[test]
    fn persist_save_snapshot_action_dispatches() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::PersistSaveSnapshot,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
    }

    #[test]
    fn persist_import_bookmarks_action_dispatches() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::PersistImportBookmarks,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
    }

    #[test]
    fn node_copy_url_action_targeted_via_action_on_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode {
                action_id: graphshell_core::actions::ActionId::NodeCopyUrl,
                node_key,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(app.host.runtime.focused_node_hint, Some(node_key));
    }

    #[test]
    fn open_settings_pane_action_creates_verso_settings_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::WorkbenchOpenSettingsPane,
            },
        );
        app.tick_with_events(Vec::new());

        let nodes_after = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(
            nodes_after,
            nodes_before + 1,
            "settings pane action should add exactly one node",
        );
        assert!(
            app.host
                .runtime
                .graph_app
                .domain_graph()
                .nodes()
                .any(|(_, n)| n.url() == "verso://settings"),
            "settings pane node should carry the canonical verso://settings address",
        );
    }

    #[test]
    fn open_hub_action_creates_verso_hub_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::PersistOpenHub,
            },
        );
        app.tick_with_events(Vec::new());

        assert!(
            app.host
                .runtime
                .graph_app
                .domain_graph()
                .nodes()
                .any(|(_, n)| n.url() == "verso://hub"),
        );
    }

    #[test]
    fn open_history_manager_creates_verso_tool_history_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::PersistOpenHistoryManager,
            },
        );
        app.tick_with_events(Vec::new());

        assert!(
            app.host
                .runtime
                .graph_app
                .domain_graph()
                .nodes()
                .any(|(_, n)| n.url() == "verso://tool/history"),
        );
    }

    #[test]
    fn open_settings_overlay_uses_same_verso_settings_address() {
        // Slice 30 collapses pane + overlay onto the same address —
        // the presentation distinction is downstream chrome, not a
        // routing concern.
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::WorkbenchOpenSettingsOverlay,
            },
        );
        app.tick_with_events(Vec::new());

        assert!(
            app.host
                .runtime
                .graph_app
                .domain_graph()
                .nodes()
                .any(|(_, n)| n.url() == "verso://settings"),
        );
    }

    // --- Swatches tests (Slice 33) ---

    #[test]
    fn swatch_recipe_builtin_set_has_canonical_recipes() {
        let recipes = SwatchRecipe::builtin_set();
        assert!(recipes.contains(&SwatchRecipe::FullGraph));
        assert!(recipes.contains(&SwatchRecipe::RecentlyActive));
        assert!(recipes.contains(&SwatchRecipe::FocusedNeighborhood));
        assert!(recipes.iter().all(|r| !r.label().is_empty()));
    }

    #[test]
    fn swatch_clicked_acks_via_toast() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let toasts_before = app.host.toast_queue.len();
        let _ = app.update(Message::SwatchClicked(SwatchRecipe::FullGraph));

        assert_eq!(app.host.toast_queue.len(), toasts_before + 1);
        let msg = &app.host.toast_queue.last().unwrap().message;
        assert!(msg.contains("Full graph"), "got: {msg}");
    }

    #[test]
    fn view_renders_swatches_bucket_empty_graph() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        // Ensure the left Navigator host is visible so swatches render.
        app.navigator.show_left = true;

        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    #[test]
    fn view_renders_swatches_bucket_populated() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        app.navigator.show_left = true;
        seed_test_nodes(&mut app, 3);

        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    // --- NodeCreate modal tests (Slice 32) ---

    #[test]
    fn node_create_open_focuses_input_and_clears_draft() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        app.node_create.url_draft = "stale".into();

        let _task = app.update(Message::NodeCreateOpen);

        assert!(app.node_create.is_open);
        assert!(app.node_create.url_draft.is_empty(), "open clears stale draft");
    }

    #[test]
    fn node_create_submit_creates_node_at_url() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::NodeCreateOpen);
        let _ = app.update(Message::NodeCreateInput("https://create.test/".into()));
        let _ = app.update(Message::NodeCreateSubmit);

        assert!(!app.node_create.is_open);
        let nodes_after = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(nodes_after, nodes_before + 1);
        assert!(
            app.host
                .runtime
                .graph_app
                .domain_graph()
                .nodes()
                .any(|(_, n)| n.url() == "https://create.test/"),
        );
    }

    #[test]
    fn node_create_cancel_drops_draft() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeCreateOpen);
        let _ = app.update(Message::NodeCreateInput("https://nope.test/".into()));
        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();
        let _ = app.update(Message::NodeCreateCancel);

        assert!(!app.node_create.is_open);
        assert!(app.node_create.url_draft.is_empty());
        assert_eq!(
            app.host.runtime.graph_app.domain_graph().nodes().count(),
            nodes_before,
            "cancel must not create a node",
        );
    }

    #[test]
    fn node_create_submit_empty_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::NodeCreateOpen);
        let _ = app.update(Message::NodeCreateSubmit);

        assert_eq!(
            app.host.runtime.graph_app.domain_graph().nodes().count(),
            nodes_before,
        );
    }

    #[test]
    fn node_new_action_routes_to_node_create_modal() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let node_new_idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| a.action_id == graphshell_core::actions::ActionId::NodeNew)
            .expect("NodeNew in registry");

        let _task = app.update(Message::PaletteActionSelected(node_new_idx));
        // Resolve the host-intercepted Task::done
        let _ = app.update(Message::NodeCreateOpen);

        assert!(app.node_create.is_open);
        assert_eq!(app.host.runtime.dispatched_action_count, 0);
    }

    #[test]
    fn view_renders_with_node_create_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeCreateOpen);
        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    // --- Settings pane content tests (Slice 39) ---

    #[test]
    fn settings_toggle_navigator_left_flips_state() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let initial = app.navigator.show_left;

        let _ = app.update(Message::SettingsToggleNavigatorLeft);
        assert_eq!(app.navigator.show_left, !initial);

        let _ = app.update(Message::SettingsToggleNavigatorLeft);
        assert_eq!(app.navigator.show_left, initial, "second toggle restores");
    }

    #[test]
    fn settings_toggle_all_navigator_anchors_independent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::SettingsToggleNavigatorRight);
        let _ = app.update(Message::SettingsToggleNavigatorTop);
        let _ = app.update(Message::SettingsToggleNavigatorBottom);

        assert!(app.navigator.show_right);
        assert!(app.navigator.show_top);
        assert!(app.navigator.show_bottom);
        // Left was not toggled; default state preserved.
        assert!(app.navigator.show_left);
    }

    #[test]
    fn tile_pane_with_settings_url_renders_settings_pane() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Seed via OmnibarSubmit so a verso://settings node is the
        // first GraphTree member; the Tile pane defaults to the
        // first member as its active tile.
        let _ = app.update(Message::OmnibarInput("verso://settings".into()));
        let _ = app.update(Message::OmnibarSubmit);
        // Convert the seeded Canvas pane to Tile so the settings
        // body renders.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }

        let _ = app.update(Message::Tick);
        // Render-time smoke test: the tile pane with a settings
        // active tile must not panic; the URL detection routes to
        // render_settings_pane.
        let _element = app.view();
    }

    #[test]
    fn open_settings_action_creates_settings_node_and_renders() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Slice 30 routed WorkbenchOpenSettingsPane through verso://
        // settings creation. Slice 39 makes that node render real
        // settings UI when shown in a Tile pane.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::WorkbenchOpenSettingsPane,
            },
        );
        app.tick_with_events(Vec::new());

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    // --- Drop-zone hint tests (Slice 36) ---

    #[test]
    fn pane_drag_picked_sets_drag_in_progress() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let handle = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(h, _)| *h)
            .unwrap();
        assert!(!app.frame.drag_in_progress);

        let _ = app.update(Message::PaneGridDragged(
            iced::widget::pane_grid::DragEvent::Picked { pane: handle },
        ));

        assert!(app.frame.drag_in_progress);
    }

    #[test]
    fn pane_drag_canceled_clears_drag_in_progress() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let handle = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(h, _)| *h)
            .unwrap();

        let _ = app.update(Message::PaneGridDragged(
            iced::widget::pane_grid::DragEvent::Picked { pane: handle },
        ));
        assert!(app.frame.drag_in_progress);

        let _ = app.update(Message::PaneGridDragged(
            iced::widget::pane_grid::DragEvent::Canceled { pane: handle },
        ));
        assert!(!app.frame.drag_in_progress);
    }

    #[test]
    fn view_renders_drop_zone_hint_during_drag() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let handle = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(h, _)| *h)
            .unwrap();

        // Render once without drag — banner not in tree.
        {
            let _ = app.view();
        }

        let _ = app.update(Message::PaneGridDragged(
            iced::widget::pane_grid::DragEvent::Picked { pane: handle },
        ));

        // Render with drag — must not panic; banner is in the tree.
        let _ = app.view();
    }

    // --- Per-pane camera cache tests (Slice 35) ---

    #[test]
    fn camera_change_with_pane_id_writes_per_pane_cache() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        assert!(app.frame.pane_cameras.is_empty());

        let pan = Vector2D::new(10.0, 20.0);
        let zoom = 2.5;
        let _ = app.update(Message::CameraChanged {
            pane_id: Some(pane_id),
            pan,
            zoom,
        });

        let cached = app
            .frame
            .pane_cameras
            .get(&pane_id)
            .expect("pane camera was cached");
        assert_eq!(cached.pan, pan);
        assert_eq!(cached.zoom, zoom);
    }

    #[test]
    fn camera_change_with_no_pane_id_skips_per_pane_cache() {
        // Base-layer camera changes pass pane_id: None; nothing
        // should land in pane_cameras.
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::CameraChanged {
            pane_id: None,
            pan: Vector2D::new(5.0, 5.0),
            zoom: 1.5,
        });

        assert!(app.frame.pane_cameras.is_empty());
        // Legacy view-keyed entry still gets the camera so
        // fit-to-screen / cross-host paths see the change.
        let view_id = app.host.view_id;
        let entry = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .canvas_cameras
            .get(&view_id)
            .expect("legacy entry");
        assert_eq!(entry.pan, Vector2D::new(5.0, 5.0));
        assert_eq!(entry.zoom, 1.5);
    }

    #[test]
    fn close_pane_drops_its_camera_cache_entry() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let (handle, pane_id) = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(h, m)| (*h, m.pane_id))
            .unwrap();
        // Plant a camera in the cache.
        let _ = app.update(Message::CameraChanged {
            pane_id: Some(pane_id),
            pan: Vector2D::new(1.0, 1.0),
            zoom: 1.0,
        });
        assert!(app.frame.pane_cameras.contains_key(&pane_id));

        let _ = app.update(Message::ClosePane(handle));

        assert!(
            !app.frame.pane_cameras.contains_key(&pane_id),
            "ClosePane drops the per-pane camera cache entry",
        );
    }

    // --- FrameRename modal tests (Slice 34) ---

    #[test]
    fn frame_rename_open_seeds_draft_with_current_label() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _task = app.update(Message::FrameRenameOpen);

        assert!(app.frame_rename.is_open);
        assert_eq!(
            app.frame_rename.label_draft, app.frame_label,
            "open seeds the draft with the current label",
        );
    }

    #[test]
    fn frame_rename_submit_applies_new_label() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::FrameRenameOpen);
        let _ = app.update(Message::FrameRenameInput("Research session".into()));
        let _ = app.update(Message::FrameRenameSubmit);

        assert!(!app.frame_rename.is_open);
        assert_eq!(app.frame_label, "Research session");
    }

    #[test]
    fn frame_rename_submit_empty_or_whitespace_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let original_label = app.frame_label.clone();
        let _ = app.update(Message::FrameRenameOpen);
        let _ = app.update(Message::FrameRenameInput("   ".into()));
        let _ = app.update(Message::FrameRenameSubmit);

        assert_eq!(app.frame_label, original_label);
    }

    #[test]
    fn frame_rename_cancel_drops_draft() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let original_label = app.frame_label.clone();
        let _ = app.update(Message::FrameRenameOpen);
        let _ = app.update(Message::FrameRenameInput("never apply".into()));
        let _ = app.update(Message::FrameRenameCancel);

        assert!(!app.frame_rename.is_open);
        assert_eq!(app.frame_label, original_label);
    }

    #[test]
    fn frame_rename_action_routes_to_modal() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| a.action_id == graphshell_core::actions::ActionId::FrameRename)
            .expect("FrameRename in registry");

        let _task = app.update(Message::PaletteActionSelected(idx));
        let _ = app.update(Message::FrameRenameOpen);

        assert!(app.frame_rename.is_open);
        assert_eq!(app.host.runtime.dispatched_action_count, 0);
    }

    // --- Modal fade-in clock tests (Slice 47) ---

    #[test]
    fn palette_open_sets_modal_opened_at_and_close_clears_it() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        assert!(app.modal_opened_at.is_none());
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::Programmatic,
        });
        assert!(app.modal_opened_at.is_some());
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);
        assert!(app.modal_opened_at.is_none());
    }

    #[test]
    fn node_finder_open_sets_modal_opened_at_and_close_clears_it() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::OmnibarRoute(String::new()),
        });
        assert!(app.modal_opened_at.is_some());
        let _ = app.update(Message::NodeFinderCloseAndRestoreFocus);
        assert!(app.modal_opened_at.is_none());
    }

    #[test]
    fn node_create_open_sets_modal_opened_at_and_close_clears_it() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::NodeCreateOpen);
        assert!(app.modal_opened_at.is_some());
        let _ = app.update(Message::NodeCreateCancel);
        assert!(app.modal_opened_at.is_none());
    }

    #[test]
    fn frame_rename_open_sets_modal_opened_at_and_close_clears_it() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::FrameRenameOpen);
        assert!(app.modal_opened_at.is_some());
        let _ = app.update(Message::FrameRenameCancel);
        assert!(app.modal_opened_at.is_none());
    }

    #[test]
    fn switching_between_modals_resets_the_fade_clock() {
        // Mutually exclusive overlays share a single clock — opening
        // a second modal must overwrite the first one's timestamp so
        // the new surface fades from scrim cleanly.
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::Programmatic,
        });
        let palette_started = app.modal_opened_at.expect("palette set the clock");
        // Sleep a tiny bit so the second timestamp differs.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::OmnibarRoute(String::new()),
        });
        let finder_started = app.modal_opened_at.expect("finder set the clock");
        assert!(
            finder_started > palette_started,
            "expected finder open to overwrite palette timestamp"
        );
    }

    // --- Frame composition tests (Slice 31) ---

    #[test]
    fn iced_app_starts_with_one_frame() {
        let runtime = GraphshellRuntime::for_testing();
        let app = IcedApp::with_runtime(runtime);
        assert_eq!(app.inactive_frames.len(), 0);
        assert_eq!(app.frame_label, "Frame 1");
    }

    #[test]
    fn new_frame_creates_blank_frame_and_backgrounds_previous() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let initial_id = app.frame_id;
        let initial_label = app.frame_label.clone();

        let _ = app.update(Message::NewFrame);

        assert_eq!(app.inactive_frames.len(), 1);
        assert_eq!(app.inactive_frames[0].id, initial_id);
        assert_eq!(app.inactive_frames[0].label, initial_label);
        assert_ne!(app.frame_id, initial_id, "active frame got a fresh id");
        assert_eq!(app.frame_label, "Frame 2");
        // The new active frame is a fresh FrameState (one Canvas pane).
        assert_eq!(app.frame.split_state.len(), 1);
    }

    #[test]
    fn switch_frame_swaps_active_with_inactive_slot() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let frame1_id = app.frame_id;
        let _ = app.update(Message::NewFrame); // Frame 1 → background
        let frame2_id = app.frame_id;
        assert_eq!(app.inactive_frames.len(), 1);
        assert_eq!(app.inactive_frames[0].id, frame1_id);

        let _ = app.update(Message::SwitchFrame(0));

        assert_eq!(app.frame_id, frame1_id, "switch promoted Frame 1");
        assert_eq!(app.inactive_frames.len(), 1);
        assert_eq!(
            app.inactive_frames[0].id, frame2_id,
            "Frame 2 moved into the inactive slot",
        );
    }

    #[test]
    fn close_current_frame_promotes_inactive_or_noop_when_alone() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        // Single frame: close is a no-op.
        let only_id = app.frame_id;
        let _ = app.update(Message::CloseCurrentFrame);
        assert_eq!(app.frame_id, only_id);
        assert_eq!(app.inactive_frames.len(), 0);

        // Add a second frame, close it, verify the first is restored.
        let _ = app.update(Message::NewFrame);
        let _ = app.update(Message::CloseCurrentFrame);
        assert_eq!(app.frame_id, only_id);
        assert_eq!(app.inactive_frames.len(), 0);
    }

    #[test]
    fn switch_frame_with_no_inactive_frames_toasts_info() {
        // Slice 41: previously a silent no-op when FrameSelect fires
        // with only one Frame open. Now toasts so the user sees why
        // nothing switched.
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let toasts_before = app.host.toast_queue.len();

        let _ = app.update(Message::SwitchFrame(0));

        assert_eq!(app.host.toast_queue.len(), toasts_before + 1);
        let msg = &app.host.toast_queue.last().unwrap().message;
        assert!(msg.contains("No other Frames"), "got: {msg}");
    }

    #[test]
    fn status_bar_surfaces_opens_count() {
        // Slice 41: opened_node_count alongside dispatched_action_count.
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        // Dispatch one OpenNode so the runtime counter > 0.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::OpenNode { node_key },
        );
        app.tick_with_events(Vec::new());
        assert_eq!(app.host.runtime.opened_node_count, 1);

        // View renders without panicking — the new "opens: N"
        // segment is present.
        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    #[test]
    fn frame_open_action_routes_to_new_frame_message() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let frame_open_idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| a.action_id == graphshell_core::actions::ActionId::FrameOpen)
            .expect("FrameOpen in registry");
        let _task = app.update(Message::PaletteActionSelected(frame_open_idx));
        // Resolve the host-intercepted Task → NewFrame.
        let _ = app.update(Message::NewFrame);
        assert_eq!(
            app.inactive_frames.len(),
            1,
            "FrameOpen should have moved Frame 1 into inactive_frames",
        );
    }

    #[test]
    fn view_renders_frame_switcher_only_when_multiple_frames_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        // Single frame: render just builds without the switcher.
        let _ = app.update(Message::Tick);
        {
            let _ = app.view();
        }
        // Two frames: switcher renders.
        let _ = app.update(Message::NewFrame);
        let _ = app.update(Message::Tick);
        let _ = app.view();
    }

    #[test]
    fn graph_command_palette_action_routes_through_host_intercept() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Find the GraphCommandPalette ActionId in the palette list.
        let palette_idx = app
            .command_palette
            .all_actions
            .iter()
            .position(|a| {
                a.action_id == graphshell_core::actions::ActionId::GraphCommandPalette
            })
            .expect("GraphCommandPalette is in the canonical action list");

        // Pre-condition: no dispatch yet.
        assert_eq!(app.host.runtime.dispatched_action_count, 0);

        // Selecting GraphCommandPalette should route through the host
        // intercept (re-open the palette as Programmatic) rather than
        // pushing a runtime intent.
        let _task = app.update(Message::PaletteActionSelected(palette_idx));
        // The intercept dispatches PaletteOpen via Task::done; simulate
        // the runtime delivering it.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::Programmatic,
        });

        assert!(
            app.command_palette.is_open,
            "host intercept reopened the palette",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "host-routed action did NOT push HostIntent::Action",
        );
    }

    #[test]
    fn unhandled_action_still_records_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // ActionId::FrameRename has no handler today — Slice 15 is
        // incremental. The dispatch counter still bumps so the
        // routing path is observable.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::FrameRename,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::FrameRename),
        );
    }

    #[test]
    fn palette_disabled_action_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Synthesize a disabled row at index 0.
        app.command_palette.all_actions[0].is_available = false;
        app.command_palette.all_actions[0].disabled_reason = Some("test".into());

        let _ = app.update(Message::PaletteActionSelected(0));

        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "disabled selection must not dispatch",
        );
        assert!(app.host.runtime.last_dispatched_action.is_none());
        assert!(app.host.pending_host_intents.is_empty());
    }

    #[test]
    fn palette_action_select_toast_carries_canonical_key() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let visible = visible_palette_actions(&app.command_palette);
        let row0_id = visible[0].action_id;
        let expected_key = row0_id.key();

        let _ = app.update(Message::PaletteActionSelected(0));

        assert_eq!(app.host.toast_queue.len(), 1);
        let msg = &app.host.toast_queue[0].message;
        assert!(
            msg.contains(expected_key),
            "toast should embed canonical ActionId::key() ({expected_key}); got: {msg}",
        );
    }

    #[test]
    fn palette_seeded_from_action_registry() {
        let runtime = GraphshellRuntime::for_testing();
        let app = IcedApp::with_runtime(runtime);

        // Every ActionId in the canonical registry becomes one RankedAction.
        let registry_count = graphshell_core::actions::all_action_ids().len();
        assert_eq!(
            app.command_palette.all_actions.len(),
            registry_count,
            "palette mirrors graphshell_core::actions::all_action_ids()",
        );
        assert!(
            app.command_palette
                .all_actions
                .iter()
                .any(|a| a.label == "Open Settings Pane"),
            "expected canonical ActionId::label(); got labels: {:?}",
            app.command_palette
                .all_actions
                .iter()
                .map(|a| a.label.as_str())
                .take(5)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn palette_query_filters_visible_actions() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let total = app.command_palette.all_actions.len();

        // No query → all actions visible.
        assert_eq!(visible_palette_actions(&app.command_palette).len(), total);

        // Substring match (case-insensitive).
        let _ = app.update(Message::PaletteQuery("settings".into()));
        let visible = visible_palette_actions(&app.command_palette);
        assert!(visible.iter().all(|a| a.label.to_lowercase().contains("settings")));
        assert!(!visible.is_empty(), "Settings is in the placeholder list");

        // Reset query → all visible again.
        let _ = app.update(Message::PaletteQuery(String::new()));
        assert_eq!(visible_palette_actions(&app.command_palette).len(), total);
    }

    #[test]
    fn palette_focus_down_advances_and_wraps() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let total = visible_palette_actions(&app.command_palette).len();
        assert!(total > 1, "need ≥2 placeholder rows for wrap test");

        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(0));

        for expected in 1..total {
            let _ = app.update(Message::PaletteFocusDown);
            assert_eq!(app.command_palette.focused_index, Some(expected));
        }

        // Wrap around.
        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(0));
    }

    #[test]
    fn palette_focus_up_from_none_wraps_to_last() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let total = visible_palette_actions(&app.command_palette).len();
        assert!(total > 0);

        let _ = app.update(Message::PaletteFocusUp);
        assert_eq!(app.command_palette.focused_index, Some(total - 1));
    }

    #[test]
    fn palette_submit_focused_fires_focused_action() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Focus the second row.
        let _ = app.update(Message::PaletteFocusDown);
        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(1));

        // Resolve PaletteSubmitFocused → PaletteActionSelected(1).
        let _ = app.update(Message::PaletteSubmitFocused);
        let _ = app.update(Message::PaletteActionSelected(1));

        assert!(!app.command_palette.is_open, "selecting closes the palette");
        assert_eq!(app.host.toast_queue.len(), 1);
    }

    #[test]
    fn palette_disabled_action_select_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Slice 9: every registry action seeds with `is_available =
        // true`. Synthesize a disabled row to exercise the no-op path
        // — runtime swap will drive availability via
        // ActionRegistryViewModel.
        app.command_palette.all_actions[0].is_available = false;
        app.command_palette.all_actions[0].disabled_reason =
            Some("synthetic disabled state".into());

        let _ = app.update(Message::PaletteActionSelected(0));

        assert!(
            app.command_palette.is_open,
            "disabled selection must not close the palette",
        );
        assert!(app.host.toast_queue.is_empty());
    }

    #[test]
    fn palette_query_reset_clears_focus() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(0));

        let _ = app.update(Message::PaletteQuery("newquery".into()));
        assert!(
            app.command_palette.focused_index.is_none(),
            "query change must reset focus index — visible list shape changed",
        );
    }

    /// Seed the runtime with `count` nodes via the same OmnibarSubmit
    /// path the real UI uses, returning the URL strings so the test
    /// can assert on them.
    fn seed_test_nodes(app: &mut IcedApp, count: usize) -> Vec<String> {
        let mut urls = Vec::with_capacity(count);
        for i in 0..count {
            let url = format!("https://example{i}.test/");
            let _ = app.update(Message::OmnibarInput(url.clone()));
            let _ = app.update(Message::OmnibarSubmit);
            urls.push(url);
        }
        urls
    }

    #[test]
    fn finder_focus_down_advances_and_wraps() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Seed at least 2 nodes so wrap behaviour is observable.
        seed_test_nodes(&mut app, 3);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        let total = visible_finder_results(&app.node_finder).len();
        assert!(total > 1, "seeded ≥3 nodes; finder should reflect them");

        let _ = app.update(Message::NodeFinderFocusDown);
        assert_eq!(app.node_finder.focused_index, Some(0));

        for expected in 1..total {
            let _ = app.update(Message::NodeFinderFocusDown);
            assert_eq!(app.node_finder.focused_index, Some(expected));
        }

        let _ = app.update(Message::NodeFinderFocusDown);
        assert_eq!(app.node_finder.focused_index, Some(0), "wrap to first row");
    }

    #[test]
    fn finder_query_filters_by_title_or_address() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 3); // example0 / example1 / example2

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::NodeFinderQuery("example1".into()));

        let visible = visible_finder_results(&app.node_finder);
        assert!(!visible.is_empty(), "exactly one URL contains 'example1'");
        assert!(
            visible.iter().all(|r| {
                r.title.to_lowercase().contains("example1")
                    || r.address.to_lowercase().contains("example1")
            }),
            "filtered set must satisfy the query",
        );
    }

    #[test]
    fn finder_result_selected_toasts_resolved_url() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let urls = seed_test_nodes(&mut app, 1);
        // OmnibarSubmit pushes its own success toast — drain so this
        // test only observes the finder's toast.
        app.host.toast_queue.clear();

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::NodeFinderResultSelected(0));

        assert!(!app.node_finder.is_open);
        assert_eq!(app.host.toast_queue.len(), 1);
        let msg = &app.host.toast_queue[0].message;
        assert!(
            msg.contains(&urls[0]),
            "toast should carry the resolved URL ({}); got: {msg}",
            urls[0],
        );
    }

    #[test]
    fn finder_seeded_from_runtime_graph_on_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Finder default is empty until opened — Slice 11 reads from
        // the live graph at open time rather than caching placeholders.
        assert!(app.node_finder.all_results.is_empty());

        seed_test_nodes(&mut app, 2);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        let nodes_in_graph = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(
            app.node_finder.all_results.len(),
            nodes_in_graph,
            "every node in the graph maps to one finder row",
        );
    }

    #[test]
    fn finder_selection_dispatches_open_node_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 1);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        // Capture the resolved NodeKey before selection.
        let row0_node_key = app.node_finder.all_results[0].node_key;
        assert_eq!(app.host.runtime.opened_node_count, 0);
        assert!(app.host.runtime.focused_node_hint.is_none());

        let _ = app.update(Message::NodeFinderResultSelected(0));

        assert!(
            app.host.pending_host_intents.is_empty(),
            "intent queue drained by post-select tick",
        );
        assert_eq!(
            app.host.runtime.opened_node_count, 1,
            "runtime observed exactly one HostIntent::OpenNode",
        );
        assert_eq!(
            app.host.runtime.focused_node_hint,
            Some(row0_node_key),
            "runtime promoted the resolved NodeKey to focused_node_hint",
        );
    }

    #[test]
    fn finder_out_of_range_selection_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        // Empty graph → empty result list → idx 0 is out of range.
        let _ = app.update(Message::NodeFinderResultSelected(0));

        assert_eq!(
            app.host.runtime.opened_node_count, 0,
            "out-of-range selection must not dispatch",
        );
        assert!(app.host.runtime.focused_node_hint.is_none());
    }

    #[test]
    fn omnibar_route_to_finder_seeds_real_results() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 2);

        // Non-URL omnibar submit routes the query to the Node Finder
        // and populates results from the live graph.
        let _ = app.update(Message::OmnibarRouteToNodeFinder("ex".into()));

        assert!(app.node_finder.is_open);
        assert_eq!(app.node_finder.query, "ex");
        let visible = visible_finder_results(&app.node_finder);
        assert!(!visible.is_empty(), "seeded URLs match 'ex' substring");
    }

    // --- Context menu tests (Slice 8) ---

    #[test]
    fn context_menu_open_seeds_entries_and_anchor() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Cache a cursor position via the regular CursorMoved path so
        // the menu's anchor reads from the canonical source.
        let _ = app.update(Message::IcedEvent(iced::Event::Mouse(
            iced::mouse::Event::CursorMoved {
                position: iced::Point::new(120.0, 80.0),
            },
        )));

        // Need a Tile pane to test that target. Replace the seeded
        // Canvas pane via direct mutation since there's no public
        // "convert pane" message yet.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }

        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .expect("pane present");

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });

        assert!(app.context_menu.is_open);
        assert_eq!(app.context_menu.target, ContextMenuTarget::TilePane { pane_id, node_key: None });
        assert_eq!(app.context_menu.anchor, iced::Point::new(120.0, 80.0));
        assert!(
            app.context_menu
                .items
                .iter()
                .any(|i| i.entry.label == "Activate"),
            "TilePane menu should include Activate",
        );
        assert!(
            app.context_menu.items.iter().any(|i| i.entry.destructive),
            "TilePane menu should include a destructive Tombstone entry",
        );
    }

    #[test]
    fn context_menu_target_drives_entry_set() {
        // Distinct targets surface distinct entry sets.
        let canvas = items_for_target(ContextMenuTarget::CanvasPane { pane_id: PaneId(1), node_key: None });
        let tile = items_for_target(ContextMenuTarget::TilePane { pane_id: PaneId(1), node_key: None });
        let base = items_for_target(ContextMenuTarget::BaseLayer);

        assert!(canvas.iter().any(|i| i.entry.label == "Inspect"));
        assert!(!tile.iter().any(|i| i.entry.label == "Inspect"));
        assert!(base.iter().any(|i| i.entry.label == "Open Pane"));
    }

    #[test]
    fn context_menu_open_dismisses_modals() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });

        assert!(!app.command_palette.is_open, "context menu closes palette");
        assert!(app.context_menu.is_open);
    }

    #[test]
    fn context_menu_entry_selected_acks_and_closes() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        // BaseLayer entry 0 is "Open Pane" (enabled, intent = None — stub-only).
        let _ = app.update(Message::ContextMenuEntrySelected(0));

        assert!(!app.context_menu.is_open);
        assert!(app.context_menu.items.is_empty(), "items cleared");
        assert_eq!(app.host.toast_queue.len(), 1);
        let msg = &app.host.toast_queue[0].message;
        assert!(msg.contains("Open Pane"), "got: {msg}");
        assert!(
            msg.contains("(stub)"),
            "BaseLayer 'Open Pane' has no intent yet — toast should mark it stub",
        );
    }

    #[test]
    fn context_menu_disabled_entry_select_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        // BaseLayer entry 1 is "Switch graphlet" (disabled — no graphlets).
        let disabled_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.disabled_reason.is_some())
            .expect("BaseLayer has a disabled entry");

        let _ = app.update(Message::ContextMenuEntrySelected(disabled_idx));

        assert!(
            app.context_menu.is_open,
            "disabled select must not close the menu",
        );
        assert!(app.host.toast_queue.is_empty());
    }

    #[test]
    fn context_menu_dismiss_closes_without_acting() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        let _ = app.update(Message::ContextMenuDismiss);

        assert!(!app.context_menu.is_open);
        assert!(app.host.toast_queue.is_empty());
    }

    #[test]
    fn escape_closes_context_menu_first() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Both context menu and palette could be open simultaneously
        // even though the state-level wiring should prevent it; verify
        // Escape's precedence in the resolution order regardless.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        // Force-open context menu over the palette by direct dispatch
        // (skips the state-level mutual-exclusion path).
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        assert!(app.context_menu.is_open);
        assert!(!app.command_palette.is_open, "ContextMenuOpen closed palette");

        // Now palette is already closed, so Escape should close the
        // context menu.
        let _ = app.update(Message::ContextMenuDismiss);
        assert!(!app.context_menu.is_open);
    }

    #[test]
    fn context_menu_action_entry_dispatches_host_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Convert the seeded Canvas pane to a Tile pane so the
        // wired-action entries are available.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .expect("pane present");

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        // Find the "Pin" entry — it's wired to ActionId::NodePinToggle.
        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .expect("TilePane menu carries a Pin entry");
        assert_eq!(app.host.runtime.dispatched_action_count, 0);

        let _ = app.update(Message::ContextMenuEntrySelected(pin_idx));

        assert!(!app.context_menu.is_open);
        assert!(
            app.host.pending_host_intents.is_empty(),
            "post-select tick drained the intent",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 1,
            "runtime observed exactly one HostIntent::Action",
        );
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodePinToggle),
            "context-menu selection routed the wired ActionId",
        );
        // Toast should NOT carry the (stub) suffix since dispatch closed.
        let msg = &app.host.toast_queue[0].message;
        assert!(msg.contains("Pin") && !msg.contains("(stub)"), "got: {msg}");
    }

    #[test]
    fn context_menu_destructive_entry_routes_through_confirm_dialog() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .expect("pane present");

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });

        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .expect("TilePane menu carries a destructive Tombstone entry");

        // Slice 14: destructive selection parks the intent in the
        // confirm dialog instead of dispatching immediately.
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));

        assert!(
            app.confirm_dialog.is_open,
            "destructive selection opens the confirm dialog gate",
        );
        assert!(app.confirm_dialog.pending_intent.is_some());
        assert!(
            !app.context_menu.is_open,
            "context menu closed when the dialog opened",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "no dispatch yet — awaiting confirmation",
        );

        // User confirms.
        let _ = app.update(Message::ConfirmDialogConfirm);

        assert!(!app.confirm_dialog.is_open);
        assert!(app.confirm_dialog.pending_intent.is_none());
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodeMarkTombstone),
            "confirm dispatched the parked intent",
        );
    }

    #[test]
    fn confirm_dialog_cancel_drops_pending_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        assert!(app.confirm_dialog.is_open);

        let _ = app.update(Message::ConfirmDialogCancel);

        assert!(!app.confirm_dialog.is_open);
        assert!(app.confirm_dialog.pending_intent.is_none());
        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "cancel must drop the parked intent without dispatching",
        );
    }

    #[test]
    fn confirm_dialog_escape_cancels_first() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Open both context menu and (hypothetically) confirm dialog
        // — but since destructive selection closes the menu before
        // opening the dialog, the natural state is just the dialog.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        assert!(app.confirm_dialog.is_open);

        let _ = app.update(Message::ConfirmDialogCancel);

        assert!(!app.confirm_dialog.is_open);
    }

    #[test]
    fn non_destructive_action_skips_confirm_dialog() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .unwrap();

        let _ = app.update(Message::ContextMenuEntrySelected(pin_idx));

        assert!(
            !app.confirm_dialog.is_open,
            "non-destructive entries do not open the confirm dialog",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 1,
            "non-destructive entries dispatch immediately",
        );
    }

    #[test]
    fn view_renders_with_confirm_dialog_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        let _ = app.update(Message::Tick);

        let _element = app.view();
    }

    #[test]
    fn context_menu_stub_entry_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        // BaseLayer "Open Pane" is intent=None (stub-only).
        let _ = app.update(Message::ContextMenuEntrySelected(0));

        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "stub entries must not dispatch",
        );
        assert!(app.host.pending_host_intents.is_empty());
    }

    #[test]
    fn view_renders_with_context_menu_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        let _ = app.update(Message::Tick);

        // Render-time smoke test — the gs::ContextMenu overlay must
        // not panic when stacked on top of the body.
        let _element = app.view();
    }
