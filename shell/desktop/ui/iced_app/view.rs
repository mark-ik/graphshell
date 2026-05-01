//! View-layer free functions extracted from `iced_app/mod.rs` —
//! Phase A of the post-Slice-39 decomposition. All `render_*`
//! helpers, hotkey detection, host-route mapping, and event
//! formatting live here; mod.rs keeps the orchestration (state
//! types, IcedApp, Message, update, view, run_application).

use super::*;

pub(super) fn render_frame_split_tree(app: &IcedApp) -> Element<'_, Message> {
    if app.frame.base_layer_active {
        render_canvas_base_layer(app)
    } else {
        pane_grid(&app.frame.split_state, |pane_handle, meta, _is_maximized| {
            let pane_label = match meta.pane_type {
                PaneType::Canvas => "Canvas",
                PaneType::Tile => "Tile pane",
            };

            // Title bar: pane label + close button.
            let title_row: Element<'_, Message> = iced::widget::row![
                text(pane_label).size(12).width(Length::Fill),
                button(text("×").size(10)).on_press(Message::ClosePane(pane_handle)),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(4)
            .into();

            let body = render_pane_body(app, meta);
            pane_grid::Content::new(body).title_bar(pane_grid::TitleBar::new(title_row))
        })
        .on_click(Message::PaneFocused)
        .on_drag(Message::PaneGridDragged)
        .on_resize(10, Message::PaneGridResized)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

/// Render the body of one Pane. Canvas panes show the graph canvas
/// program; Tile panes show a tile-tab bar over a placeholder body.
///
/// The body is wrapped in a `mouse_area` whose `on_right_press` opens
/// the context menu against the appropriate `ContextMenuTarget`. The
/// anchor (cursor position) is read in the message handler from
/// `IcedHost.cursor_position`.
pub(super) fn render_pane_body<'a>(app: &'a IcedApp, meta: &PaneMeta) -> Element<'a, Message> {
    let inner: Element<'a, Message> = match meta.pane_type {
        PaneType::Canvas => {
            let program =
                graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
            let _: &GraphCanvasProgram = &program;
            let graph: Element<'_, crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage> =
                canvas(program).width(Length::Fill).height(Length::Fill).into();
            // Capture the pane id so RightClicked can target this pane
            // and CameraChanged can route into FrameState.pane_cameras.
            let pane_id = meta.pane_id;
            graph.map(move |gcm| match gcm {
                crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
                    Message::CameraChanged {
                        pane_id: Some(pane_id),
                        pan,
                        zoom,
                    }
                }
                crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage::RightClicked { hit_node } => {
                    Message::ContextMenuOpen {
                        target: ContextMenuTarget::CanvasPane {
                            pane_id,
                            node_key: hit_node,
                        },
                    }
                }
            })
        }
        PaneType::Tile => render_tile_pane_body(app, meta),
    };

    // Slice 17: canvas panes handle right-click natively in the
    // canvas Program (hit-test populates node_key). Tile panes still
    // route right-click via the outer mouse_area since they don't
    // have an inner Program; tile-tab right-click hit-test lands
    // when the tile bar exposes per-tab targets.
    match meta.pane_type {
        PaneType::Canvas => inner,
        PaneType::Tile => mouse_area(inner)
            .on_right_press(Message::ContextMenuOpen {
                target: ContextMenuTarget::TilePane {
                    pane_id: meta.pane_id,
                    node_key: None,
                },
            })
            .into(),
    }
}

/// Render the settings pane content — Slice 39. Shown inside any
/// tile pane whose active tile's URL starts with `verso://settings`.
/// Today exposes a small set of host-side toggles (Navigator host
/// visibility); per-section settings (verso://settings/physics,
/// /frames, etc.) inspect the URL suffix to decide what to render.
pub(super) fn render_settings_pane(app: &IcedApp) -> Element<'_, Message> {
    let header = text("Settings").size(15);
    let nav_section = text("Navigator hosts").size(13);
    let nav_left = iced::widget::checkbox(app.navigator.show_left)
        .label("Left sidebar")
        .on_toggle(|_| Message::SettingsToggleNavigatorLeft)
        .size(14);
    let nav_right = iced::widget::checkbox(app.navigator.show_right)
        .label("Right sidebar")
        .on_toggle(|_| Message::SettingsToggleNavigatorRight)
        .size(14);
    let nav_top = iced::widget::checkbox(app.navigator.show_top)
        .label("Top toolbar")
        .on_toggle(|_| Message::SettingsToggleNavigatorTop)
        .size(14);
    let nav_bottom = iced::widget::checkbox(app.navigator.show_bottom)
        .label("Bottom toolbar")
        .on_toggle(|_| Message::SettingsToggleNavigatorBottom)
        .size(14);

    let footer = text(
        "Per-section settings (physics, frames, hub) reach this same \
         pane via verso://settings/<section>; richer controls land \
         per section as the host wires them.",
    )
    .size(11);

    container(
        scrollable(
            iced::widget::column![header, nav_section, nav_left, nav_right, nav_top, nav_bottom, footer]
                .spacing(8)
                .padding(12),
        )
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

/// Render the body of a Tile pane — Slice 29 wires real graphlet
/// projection per the iced jump-ship plan §S5.
///
/// Currently the tile list comes from `runtime.graph_tree.members()`
/// (the same source the Tree Spine uses). When per-pane graphlet
/// scoping lands (`Pane::graphlet_id` per spec §3 + a graphlet
/// authority that returns active tiles per-graphlet), this swaps to
/// `view_model.active_tiles_for(pane.graphlet_id)` with no shape
/// change to the rendering pipeline.
///
/// Each tab carries the resolved `NodeKey`:
/// - left-click → `TileTabSelected` → `HostIntent::OpenNode`
/// - close `×` → `TileTabClosed` → toast (real
///   `LifecycleIntent::ToggleTilePresentationState` lands when the
///   graphlet authority surfaces it)
/// - right-click → `ContextMenuOpen { TilePane { pane_id,
///   node_key: Some(...) }}` — finishes the deferred Slice 21
///   wiring where `node_key` had to be `None` for lack of tile data.
pub(super) fn render_tile_pane_body<'a>(app: &'a IcedApp, meta: &PaneMeta) -> Element<'a, Message> {
    let pane_id = meta.pane_id;
    let tiles = tiles_for_pane(app);

    if tiles.is_empty() {
        let body = container(
            text("No tiles in this graphlet — open a node from the Tree Spine \
                  or via Ctrl+P to populate the tab bar.")
                .size(12),
        )
        .center(Length::Fill);
        // Keep the empty pane right-clickable so the user can still
        // open the pane's context menu. node_key is None because
        // there's no tile under the cursor.
        return mouse_area(body)
            .on_right_press(Message::ContextMenuOpen {
                target: ContextMenuTarget::TilePane {
                    pane_id,
                    node_key: None,
                },
            })
            .into();
    }

    // Build a per-tab NodeKey vec so the right-click and select
    // closures can index by tab idx without re-querying the runtime.
    let tab_keys: Vec<graphshell_core::graph::NodeKey> =
        tiles.iter().map(|(k, _)| *k).collect();
    let tab_keys_for_select = tab_keys.clone();
    let tab_keys_for_close = tab_keys.clone();
    let tab_keys_for_right = tab_keys.clone();

    let mut tabs = TileTabs::new();
    for (_, label) in &tiles {
        tabs = tabs.push(TileTab::new(label.clone()));
    }
    let tabs = tabs
        .selected(Some(0))
        .on_select(move |i| Message::TileTabSelected {
            pane_id,
            node_key: tab_keys_for_select[i],
        })
        .on_close(move |i| Message::TileTabClosed {
            pane_id,
            node_key: tab_keys_for_close[i],
        })
        .on_right_click(move |i| Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane {
                pane_id,
                node_key: Some(tab_keys_for_right[i]),
            },
        });

    let active_label = tiles
        .first()
        .map(|(_, l)| l.clone())
        .unwrap_or_else(|| "—".into());

    // Slice 39: detect verso://settings URLs on the active tile and
    // render the settings pane content instead of the placeholder.
    // Other tile-content variants (WebViewSurface, middlenet viewer)
    // are downstream tile-render-mode work; for now settings is the
    // first concrete content view.
    let active_url = tiles.first().and_then(|(node_key, _)| {
        app.host
            .runtime
            .graph_app
            .domain_graph()
            .get_node(*node_key)
            .map(|n| n.url().to_string())
    });
    let tile_body: Element<'a, Message> = match active_url.as_deref() {
        Some(url) if url.starts_with("verso://settings") => render_settings_pane(app),
        _ => container(
            text(format!("Tile body — active: {active_label}")).size(12),
        )
        .center(Length::Fill)
        .into(),
    };

    iced::widget::column![Element::from(tabs), tile_body]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

/// Resolve the tile list for a tile pane. Slice 29: defaults to the
/// runtime's `GraphTree` membership (every node in the workbench).
/// Per-pane graphlet scoping (`Pane::graphlet_id` + a graphlet
/// authority) is the next graphlet-projection slice.
pub(super) fn tiles_for_pane(
    app: &IcedApp,
) -> Vec<(graphshell_core::graph::NodeKey, String)> {
    let runtime = &app.host.runtime;
    runtime
        .graph_tree
        .members()
        .map(|(node_key, _entry)| {
            let label = runtime
                .graph_app
                .domain_graph()
                .get_node(*node_key)
                .map(|n| {
                    if n.title.is_empty() {
                        n.url().to_string()
                    } else {
                        n.title.clone()
                    }
                })
                .unwrap_or_else(|| format!("n{}", node_key.index()));
            (*node_key, label)
        })
        .collect()
}

/// Canvas base layer — rendered when the Frame has zero Panes.
///
/// This is the same `GraphCanvasProgram` used inside Canvas Panes;
/// per spec §2.3 the base layer is a distinct code branch, not a
/// degenerate Pane. Wrapped in a `mouse_area` so right-click opens the
/// `ContextMenuTarget::BaseLayer` menu.
pub(super) fn render_canvas_base_layer(app: &IcedApp) -> Element<'_, Message> {
    let program = graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
    let _: &GraphCanvasProgram = &program;
    let graph: Element<'_, crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage> =
        canvas(program).width(Length::Fill).height(Length::Fill).into();
    // Slice 17: the canvas program now handles right-click natively
    // and runs hit-test. Empty-space right-click still falls through
    // to BaseLayer; node-on right-click would currently surface
    // CanvasPane semantics, but the base layer has no pane id so we
    // route every right-click to BaseLayer for now. A later slice
    // can introduce a `BaseLayerWithNode { node_key }` target.
    graph
        .map(|gcm| match gcm {
            crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
                // Slice 35: base layer carries pane_id: None — it has
                // no associated PaneId; only the legacy view-keyed
                // entry receives the camera persist.
                Message::CameraChanged {
                    pane_id: None,
                    pan,
                    zoom,
                }
            }
            crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage::RightClicked { .. } => {
                Message::ContextMenuOpen {
                    target: ContextMenuTarget::BaseLayer,
                }
            }
        })
}

// ---------------------------------------------------------------------------
// Navigator host rendering — Slice 4 (structural layout)
// ---------------------------------------------------------------------------

/// Which edge of the workbench a Navigator host is anchored to.
///
/// Left / Right → sidebar form factor (vertical column, fixed width).
/// Top / Bottom → toolbar form factor (horizontal row, fixed height).
/// Per [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigatorAnchor {
    Left,
    Right,
    Top,
    Bottom,
}

/// Render one Navigator host slot with stub Presentation Buckets.
///
/// Per spec §6: each host renders the three canonical buckets — Tree
/// Spine, Swatches, Activity Log — in a layout appropriate for its
/// form factor. This slice renders structural stubs; real bucket content
/// (lazy+scrollable trees, canvas swatch grid, event stream) lands once
/// the Navigator domain layer is wired (S5).
pub(super) fn render_navigator_host(app: &IcedApp, anchor: NavigatorAnchor) -> Element<'_, Message> {
    // Tree Spine bucket — Slice 20 reads from the runtime's GraphTree
    // and renders one row per member. Each row is a button that
    // dispatches `Message::TreeSpineNodeClicked(node_key)` → the
    // runtime promotes the node to focused via HostIntent::OpenNode.
    let tree_spine: Element<'_, Message> = render_tree_spine_bucket(app);

    // Swatches bucket — Slice 33 renders one compact canvas card per
    // built-in projection recipe. Per
    // [`iced_composition_skeleton_spec.md` §6.2](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    // virtualized grid of `canvas::Program` instances at the
    // `RenderProfile::Swatch` profile.
    let swatches: Element<'_, Message> = render_swatches_bucket(app);

    // Activity Log bucket — Slice 27 reads from the host's bounded
    // RecordingObserver and renders one row per UxEvent in
    // most-recent-first order. Per
    // [`iced_composition_skeleton_spec.md` §6.3](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    // event-stream view of recent runtime activity.
    let activity_log: Element<'_, Message> = render_activity_log_bucket(app);

    match anchor {
        NavigatorAnchor::Left | NavigatorAnchor::Right => {
            // Sidebar form factor: vertical column, fixed width.
            container(
                iced::widget::column![tree_spine, swatches, activity_log]
                    .spacing(4)
                    .height(Length::Fill),
            )
            .width(Length::Fixed(180.0))
            .height(Length::Fill)
            .padding(6)
            .into()
        }
        NavigatorAnchor::Top | NavigatorAnchor::Bottom => {
            // Toolbar form factor: horizontal row, fixed height.
            container(
                iced::widget::row![tree_spine, swatches, activity_log]
                    .spacing(4)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fixed(120.0))
            .padding(6)
            .into()
        }
    }
}

/// Render the CommandBar slot omnibar. Per
/// [`iced_omnibar_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md).
///
/// Slice 2: structural layout with placeholder sub-widgets. Real
/// Navigator projections (scope badge content, graphlet chip, settings
/// button routing, sync status) land in S4 when those surfaces exist.
pub(super) fn render_command_bar(app: &IcedApp) -> Element<'_, Message> {
    let scope_badge = text("–").size(12);

    let center: Element<'_, Message> = match app.omnibar.mode {
        OmnibarMode::Display => {
            let location = app
                .last_view_model
                .as_ref()
                .map(|vm| vm.toolbar.location.as_str())
                .unwrap_or("—");
            text(location).size(14).width(Length::Fill).into()
        }
        OmnibarMode::Input => text_input("Enter URL or search…", &app.omnibar.draft)
            .id(iced::widget::Id::new(OMNIBAR_INPUT_ID))
            .on_input(Message::OmnibarInput)
            .on_submit(Message::OmnibarSubmit)
            .size(14)
            .padding(4)
            .width(Length::Fill)
            .into(),
    };

    let settings_stub = text("⚙").size(14);
    let sync_stub = text("◉").size(12);

    iced::widget::row![scope_badge, center, settings_stub, sync_stub,]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
}

/// Render the Command Palette modal. Per
/// [`iced_command_palette_spec.md` §2.2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
///
/// Slice 7 renders real result rows from the (placeholder) action list,
/// with focused-state highlighting and click handlers per row. Disabled
/// rows render dimmed and accept no clicks (`on_press_maybe(None)`).
/// Arrow-key navigation routes through `PaletteFocusUp/Down`; Enter
/// fires the focused row via `PaletteSubmitFocused`.
pub(super) fn render_command_palette(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input("Type a command or search…", &app.command_palette.query)
        .id(iced::widget::Id::new(PALETTE_INPUT_ID))
        .on_input(Message::PaletteQuery)
        .on_submit(Message::PaletteSubmitFocused)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    let visible = visible_palette_actions(&app.command_palette);
    let focused = app.command_palette.focused_index;

    let results: Element<'_, Message> = if visible.is_empty() {
        let empty_label = if app.command_palette.query.is_empty() {
            "No actions available."
        } else {
            "No matching actions."
        };
        container(text(empty_label).size(12))
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let rows: Vec<Element<'_, Message>> = visible
            .iter()
            .enumerate()
            .map(|(i, action)| render_palette_row(i, action, focused == Some(i)))
            .collect();
        scrollable(column(rows).spacing(2).padding(4))
            .height(Length::Fill)
            .into()
    };

    let footer = text("Esc to dismiss · ↑/↓ to navigate · Enter to run").size(11);

    let body = iced::widget::column![
        text("Command Palette").size(13),
        input,
        divider,
        results,
        footer,
    ]
    .spacing(8)
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill);

    Modal::new(body)
        .on_blur(Message::PaletteCloseAndRestoreFocus)
        .max_width(640.0)
        .max_height(480.0)
        .into()
}

/// One row in the Command Palette ranked-action list.
///
/// Layout: label (filling, bold-ish via size) on the left, optional
/// keybinding right-aligned. Description (if present) appears beneath
/// the label at smaller size. Disabled rows pass `None` to
/// `on_press_maybe`; focused rows get a brighter background.
pub(super) fn render_palette_row<'a>(
    idx: usize,
    action: &'a RankedAction,
    is_focused: bool,
) -> Element<'a, Message> {
    // Header line: label + optional keybinding.
    let label_el: Element<'a, Message> = text(action.label.as_str()).size(13).width(Length::Fill).into();
    let header: Element<'a, Message> = if let Some(kb) = action.keybinding.as_deref() {
        iced::widget::row![label_el, text(kb).size(11)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        label_el
    };

    // Optional description line.
    let body: Element<'a, Message> = match action.description.as_deref() {
        Some(desc) if !desc.is_empty() => iced::widget::column![header, text(desc).size(11)]
            .spacing(2)
            .into(),
        _ => header,
    };

    let msg: Option<Message> = if action.is_available {
        Some(Message::PaletteActionSelected(idx))
    } else {
        None
    };

    let is_disabled = !action.is_available;

    button(body)
        .on_press_maybe(msg)
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            let bg = if is_focused {
                Some(pal.primary.weak.color.into())
            } else if hovered && !is_disabled {
                Some(tokens::HOVER_OVERLAY_SUBTLE.into())
            } else {
                None
            };
            let text_color = if is_disabled {
                iced::Color {
                    a: pal.background.base.text.a * 0.4,
                    ..pal.background.base.text
                }
            } else if is_focused {
                pal.primary.weak.text
            } else {
                pal.background.base.text
            };
            iced::widget::button::Style {
                background: bg,
                text_color,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the Node Finder modal. Per
/// [`iced_node_finder_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
///
/// Slice 7 renders real result rows from the (placeholder) result list
/// with click handlers and focused-state highlighting. Arrow-key nav
/// routes through `NodeFinderFocusUp/Down`; Enter fires the focused row
/// via `NodeFinderSubmitFocused`.
pub(super) fn render_node_finder(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input(
        "Search nodes by title, tag, URL, or content…",
        &app.node_finder.query,
    )
    .id(iced::widget::Id::new(NODE_FINDER_INPUT_ID))
    .on_input(Message::NodeFinderQuery)
    .on_submit(Message::NodeFinderSubmitFocused)
    .size(14)
    .padding(6)
    .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    let visible = visible_finder_results(&app.node_finder);
    let focused = app.node_finder.focused_index;

    let results: Element<'_, Message> = if visible.is_empty() {
        let empty_label = if app.node_finder.query.is_empty() {
            "No recently-active nodes yet."
        } else {
            "No matching nodes."
        };
        container(text(empty_label).size(12))
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let rows: Vec<Element<'_, Message>> = visible
            .iter()
            .enumerate()
            .map(|(i, result)| render_finder_row(i, result, focused == Some(i)))
            .collect();
        scrollable(column(rows).spacing(2).padding(4))
            .height(Length::Fill)
            .into()
    };

    let footer = text("Esc to dismiss · ↑/↓ to navigate · Enter to open").size(11);

    let body = iced::widget::column![
        text("Node Finder").size(13),
        input,
        divider,
        results,
        footer,
    ]
    .spacing(8)
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill);

    Modal::new(body)
        .on_blur(Message::NodeFinderCloseAndRestoreFocus)
        .max_width(640.0)
        .max_height(480.0)
        .into()
}

/// One row in the Node Finder result list.
///
/// Layout: title (filling) + node-type chip on the right; address
/// (smaller, dimmer) beneath the title. The match-source badge from
/// the spec is folded into the type chip until styling tokens land.
pub(super) fn render_finder_row<'a>(
    idx: usize,
    result: &'a NodeFinderResult,
    is_focused: bool,
) -> Element<'a, Message> {
    let title_el: Element<'a, Message> = text(result.title.as_str()).size(13).width(Length::Fill).into();
    let header: Element<'a, Message> = iced::widget::row![
        title_el,
        text(result.node_type.as_str()).size(10),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into();

    let body = iced::widget::column![header, text(result.address.as_str()).size(11)]
        .spacing(2);

    button(body)
        .on_press(Message::NodeFinderResultSelected(idx))
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            let bg = if is_focused {
                Some(pal.primary.weak.color.into())
            } else if hovered {
                Some(tokens::HOVER_OVERLAY_SUBTLE.into())
            } else {
                None
            };
            let text_color = if is_focused {
                pal.primary.weak.text
            } else {
                pal.background.base.text
            };
            iced::widget::button::Style {
                background: bg,
                text_color,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the FrameRename label-input modal — Slice 34. Visible
/// whenever `FrameRenameState::is_open` is true. Click-outside
/// (`Modal::on_blur`) and Escape both fire `FrameRenameCancel`;
/// Enter on the text input fires `FrameRenameSubmit`.
pub(super) fn render_frame_rename_modal(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Rename frame").size(15);
    let body = text(format!(
        "Renaming \"{}\". Empty submissions are no-ops.",
        app.frame_label
    ))
    .size(13);
    let input = text_input("Frame label", &app.frame_rename.label_draft)
        .id(iced::widget::Id::new(FRAME_RENAME_INPUT_ID))
        .on_input(Message::FrameRenameInput)
        .on_submit(Message::FrameRenameSubmit)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::FrameRenameCancel)
        .padding([6, 14]);
    let apply = button(text("Apply").size(13))
        .on_press(Message::FrameRenameSubmit)
        .padding([6, 14]);

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        apply,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = iced::widget::column![title, body, input, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::FrameRenameCancel)
        .max_width(420.0)
        .into()
}

/// Render the NodeCreate URL-input modal — Slice 32. Visible
/// whenever `NodeCreateState::is_open` is true. Click-outside
/// (`Modal::on_blur`) and Escape both fire `NodeCreateCancel`;
/// Enter on the text input fires `NodeCreateSubmit`.
pub(super) fn render_node_create_modal(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Create node").size(15);
    let body = text("Enter a URL or address to open as a new node.").size(13);
    let input = text_input("https://…", &app.node_create.url_draft)
        .id(iced::widget::Id::new(NODE_CREATE_INPUT_ID))
        .on_input(Message::NodeCreateInput)
        .on_submit(Message::NodeCreateSubmit)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::NodeCreateCancel)
        .padding([6, 14]);
    let create = button(text("Create").size(13))
        .on_press(Message::NodeCreateSubmit)
        .padding([6, 14]);

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        create,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = iced::widget::column![title, body, input, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::NodeCreateCancel)
        .max_width(480.0)
        .into()
}

/// Render the confirmation modal that gates destructive intents.
/// Shown when `ConfirmDialogState::is_open` is `true`; click-outside
/// (`Modal::on_blur`) and Escape both fire `ConfirmDialogCancel`. Per
/// [`iced_command_palette_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
pub(super) fn render_confirm_dialog(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Confirm destructive action").size(15);
    let body = text(format!(
        "{} cannot be undone. Continue?",
        app.confirm_dialog.action_label
    ))
    .size(13);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::ConfirmDialogCancel)
        .padding([6, 14]);
    let confirm = button(text("Confirm").size(13))
        .on_press(Message::ConfirmDialogConfirm)
        .padding([6, 14])
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(if hovered {
                    pal.danger.strong.color.into()
                } else {
                    pal.danger.base.color.into()
                }),
                text_color: pal.danger.strong.text,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        confirm,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = iced::widget::column![title, body, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::ConfirmDialogCancel)
        .max_width(420.0)
        .into()
}

/// Render the right-click context menu using `gs::ContextMenu`. The
/// widget itself does the positioning (via `pin` at the recorded
/// anchor) and the overlay layering (full-viewport dismiss area
/// behind an opaque menu panel). The host-side `ContextMenuItem`
/// pairs the display entry with an optional dispatch payload; only
/// the entry half is handed to the widget.
pub(super) fn render_context_menu(app: &IcedApp) -> Element<'_, Message> {
    let mut menu = ContextMenu::new(app.context_menu.anchor)
        .on_select(Message::ContextMenuEntrySelected)
        .on_dismiss(Message::ContextMenuDismiss);
    for item in &app.context_menu.items {
        menu = menu.push(item.entry.clone());
    }
    menu.into()
}

/// Is this iced event the "focus the omnibar" hotkey?
/// Ctrl+L (Cmd+L on macOS via `Modifiers::command()`). Consumed at
/// the app level — never reaches the runtime's `HostEvent` translation.
pub(super) fn is_omnibar_focus_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => c.as_ref().eq_ignore_ascii_case("l") && modifiers.command(),
        _ => false,
    }
}

/// Is this iced event the "open Command Palette" hotkey?
/// Ctrl+Shift+P (Zed/VSCode-shaped). Per
/// [`iced_command_palette_spec.md` §2.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
pub(super) fn is_command_palette_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event the "open Node Finder" hotkey?
/// Ctrl+P **without** Shift (Zed/VSCode-shaped). Per
/// [`iced_node_finder_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
pub(super) fn is_node_finder_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && !modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event an Escape keypress?
pub(super) fn is_escape_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        })
    )
}

/// Is this iced event an ArrowDown keypress?
pub(super) fn is_arrow_down_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
            ..
        })
    )
}

/// Is this iced event an ArrowUp keypress?
pub(super) fn is_arrow_up_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
            ..
        })
    )
}

/// Does `s` look like a URL or bare hostname?
///
/// Per [`iced_omnibar_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md):
/// explicit scheme (`://`) → URL; no spaces + contains `.` → bare
/// host. Everything else → non-URL-shaped → route to Node Finder.
pub(super) fn is_url_shaped(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if s.contains("://") {
        return true;
    }
    !s.contains(' ') && s.contains('.')
}

/// Slice 28 host-side intercept for `ActionId`s whose effect is
/// opening or toggling an iced-owned overlay or rearranging
/// host-side composition state. Returns `Some(Message)` when the
/// host should handle the action directly; `None` lets the caller
/// fall through to `HostIntent::Action` runtime dispatch.
///
/// Slice 31 extends this with Frame switcher routing — Frame
/// composition lives in `IcedApp` (`frame`, `inactive_frames`),
/// not the runtime, so Frame* actions intercept here.
pub(super) fn host_routed_action(
    action_id: graphshell_core::actions::ActionId,
) -> Option<Message> {
    use graphshell_core::actions::ActionId;
    match action_id {
        ActionId::GraphCommandPalette => Some(Message::PaletteOpen {
            origin: PaletteOrigin::Programmatic,
        }),
        // GraphRadialMenu was retired (see iced_command_palette_spec.md
        // §7.4). Re-introducing it is part of the input-subsystem
        // rework, not a host-route today.

        // Slice 31: Frame composition lives host-side.
        ActionId::FrameOpen => Some(Message::NewFrame),
        ActionId::FrameDelete => Some(Message::CloseCurrentFrame),
        // FrameSelect cycles to the next frame. The caller can pre-
        // compute the target index, but the simplest dispatch is a
        // sentinel: SwitchFrame(0) (the most-recently-backgrounded
        // frame). A future picker modal can route via SwitchFrame(idx)
        // for explicit selection.
        ActionId::FrameSelect => Some(Message::SwitchFrame(0)),
        // Slice 34: rename modal owns the active Frame's label.
        ActionId::FrameRename => Some(Message::FrameRenameOpen),

        // Slice 32: NodeCreate modal lives host-side; both NodeNew
        // and NodeNewAsTab open the same URL-input modal. The
        // pane-vs-tab distinction is downstream (the tab semantics
        // would route through workbench-routing once the pane
        // policy lands).
        ActionId::NodeNew | ActionId::NodeNewAsTab => Some(Message::NodeCreateOpen),
        _ => None,
    }
}

/// Emit a UX event onto the runtime's observer registry. Centralized
/// so every emission site has identical borrow shape — `&self.host.runtime`
/// is enough; emit() takes `&self`. Per
/// [`ux_observability`](
/// ../../../crates/graphshell-core/src/ux_observability.rs).
pub(super) fn emit_ux_event(app: &IcedApp, event: graphshell_core::ux_observability::UxEvent) {
    app.host.runtime.ux_observers.emit(event);
}

/// Render the drop-zone hint banner — Slice 36 / 38. Visible only
/// while a pane drag is in progress
/// (`FrameState::drag_in_progress == true`, between Picked and
/// Dropped/Canceled). Pane_grid handles the drop logic; this banner
/// is a visible cue describing the drop targets.
///
/// Slice 38 modulates the banner's background alpha by a sine pulse
/// computed from the host's `startup_instant`, so the banner
/// breathes (period 1200ms) while the drag is active. The base
/// color is the theme's primary-weak; alpha ramps `[0.45, 0.95]`.
pub(super) fn render_drop_zone_hint(pulse: f32) -> Element<'static, Message> {
    let hint = text(
        "Dragging — drop on a pane edge to split, on the center to swap panes",
    )
    .size(11);
    // Map pulse [0,1] → alpha [0.45, 0.95] so the banner stays
    // visible at trough but is more opaque at crest.
    let alpha = 0.45 + 0.50 * pulse;
    container(hint)
        .padding([3, 8])
        .width(Length::Fill)
        .height(Length::Fixed(22.0))
        .center_y(Length::Fill)
        .style(move |theme: &iced::Theme| {
            let pal = theme.palette();
            let bg = iced::Color {
                a: alpha,
                ..pal.primary.weak.color
            };
            container::Style {
                background: Some(bg.into()),
                text_color: Some(pal.primary.weak.text),
                ..Default::default()
            }
        })
        .into()
}

/// Render the Frame switcher bar — Slice 31. Visible only when
/// there's more than one Frame open. Each Frame is a small button
/// labeled by `frame_label`; the active Frame is highlighted; a
/// trailing "+" button creates a new Frame.
pub(super) fn render_frame_switcher(app: &IcedApp) -> Element<'_, Message> {
    let mut row = iced::widget::row![
        text(format!("{} (active)", app.frame_label)).size(11),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    for (idx, frame) in app.inactive_frames.iter().enumerate() {
        let label = frame.label.clone();
        row = row.push(
            button(text(label).size(11))
                .on_press(Message::SwitchFrame(idx))
                .padding([2, 8])
                .style(|theme: &iced::Theme, status| {
                    let pal = theme.palette();
                    let hovered = matches!(
                        status,
                        iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed
                    );
                    iced::widget::button::Style {
                        background: if hovered {
                            Some(tokens::HOVER_OVERLAY_STRONG.into())
                        } else {
                            None
                        },
                        text_color: pal.background.base.text,
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
        );
    }

    row = row.push(iced::widget::Space::new().width(Length::Fill));
    row = row.push(
        button(text("+").size(11))
            .on_press(Message::NewFrame)
            .padding([2, 8]),
    );
    if !app.inactive_frames.is_empty() {
        row = row.push(
            button(text("×").size(11))
                .on_press(Message::CloseCurrentFrame)
                .padding([2, 8]),
        );
    }

    container(row)
        .padding([3, 8])
        .width(Length::Fill)
        .height(Length::Fixed(22.0))
        .style(|theme: &iced::Theme| {
            let pal = theme.palette();
            container::Style {
                background: Some(
                    tokens::chrome_band(
                        pal.background.base.text,
                        tokens::CHROME_BAND_MEDIUM,
                    )
                    .into(),
                ),
                ..Default::default()
            }
        })
        .into()
}

/// Render the Tree Spine bucket — Navigator's left-rail "structural
/// list" of nodes in the workbench's GraphTree. Per
/// [`iced_composition_skeleton_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
///
/// Slice 20 wiring: read from `runtime.graph_tree.members()` and emit
/// one button per member with the resolved title (from the domain
/// graph). Click → `Message::TreeSpineNodeClicked(node_key)` → push
/// `HostIntent::OpenNode { node_key }`. Lifecycle / Active+Inactive
/// toggles, indentation by topology depth, and `lazy` virtualization
/// are subsequent slices once their domain hooks are wired.
pub(super) fn render_tree_spine_bucket(app: &IcedApp) -> Element<'_, Message> {
    let runtime = &app.host.runtime;
    let header: Element<'_, Message> = text("Tree Spine")
        .size(11)
        .width(Length::Fill)
        .into();

    let member_count = runtime.graph_tree.member_count();
    if member_count == 0 {
        return scrollable(
            iced::widget::column![header, text("  ○ no members yet").size(11)].spacing(4),
        )
        .height(Length::FillPortion(2))
        .into();
    }

    // Collect (NodeKey, label) pairs so the borrow on graph_tree is
    // dropped before the column builder consumes the strings.
    let members: Vec<(graphshell_core::graph::NodeKey, String)> = runtime
        .graph_tree
        .members()
        .map(|(node_key, _entry)| {
            let label = runtime
                .graph_app
                .domain_graph()
                .get_node(*node_key)
                .map(|n| {
                    if n.title.is_empty() {
                        n.url().to_string()
                    } else {
                        n.title.clone()
                    }
                })
                .unwrap_or_else(|| format!("n{}", node_key.index()));
            (*node_key, label)
        })
        .collect();

    let rows: Vec<Element<'_, Message>> = members
        .into_iter()
        .map(|(node_key, label)| tree_spine_row(node_key, label))
        .collect();

    let mut spine = iced::widget::column![header];
    for row in rows {
        spine = spine.push(row);
    }

    scrollable(spine.spacing(2).padding([2, 0]))
        .height(Length::FillPortion(2))
        .into()
}

/// Render the Swatches bucket — Navigator's middle-row "live
/// projections at a glance" surface. Per
/// [`iced_composition_skeleton_spec.md` §6.2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// virtualized grid of compact canvas instances, one per recipe.
///
/// Slice 33 ships a vertical stack of three built-in recipe cards
/// (FullGraph / RecentlyActive / FocusedNeighborhood). Each card is
/// a `gs::*`-clean composition: a small canvas widget showing the
/// graph at compact size, a label below, and a click-area that
/// dispatches `SwatchClicked(recipe)`.
///
/// Layout deviates from the spec's wrap_horizontally because vertical
/// stacking matches how the bucket lives inside a sidebar Navigator
/// host (180px wide). When a Navigator host opens in the wider
/// Top/Bottom toolbar configuration, the layout adapts via
/// FillPortion height — the card-internals don't change shape.
///
/// Real per-recipe scene scoping (filtered nodes, lens overrides)
/// lands when the projection-recipe authority is wired; the canvas
/// rendering path below stays.
pub(super) fn render_swatches_bucket(app: &IcedApp) -> Element<'_, Message> {
    let header: Element<'_, Message> =
        text("Swatches").size(11).width(Length::Fill).into();

    let nodes_count = app.host.runtime.graph_app.domain_graph().nodes().count();
    if nodes_count == 0 {
        return scrollable(
            iced::widget::column![header, text("  — no recipes (graph is empty)").size(11)]
                .spacing(4),
        )
        .height(Length::FillPortion(1))
        .into();
    }

    let mut col = iced::widget::column![header];
    for recipe in SwatchRecipe::builtin_set() {
        col = col.push(render_swatch_card(app, *recipe));
    }

    scrollable(col.spacing(6).padding([2, 0]))
        .height(Length::FillPortion(1))
        .into()
}

/// Render one swatch card. Layout: a 60px-tall canvas frame on top
/// (showing the graph in miniature), then a click-button under it
/// labeled with the recipe name.
pub(super) fn render_swatch_card<'a>(app: &'a IcedApp, recipe: SwatchRecipe) -> Element<'a, Message> {
    // Slice 33: every recipe currently shares the same scene input
    // (full graph). When per-recipe scoping lands, this swap to
    // `recipe.scene_input_for(graph_app, view_id)` — single call site
    // change.
    let program = graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
    let _: &GraphCanvasProgram = &program;
    let canvas_widget: Element<'_, crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage> =
        canvas(program)
            .width(Length::Fill)
            .height(Length::Fixed(60.0))
            .into();
    // Swatch canvases swallow camera-changed and right-clicked events
    // for now — they don't have their own pane id and right-click
    // would compete with the swatch click handler. Map to Tick (no-op).
    let canvas_inner: Element<'_, Message> = canvas_widget.map(|_| Message::Tick);

    let label = button(text(recipe.label()).size(11).width(Length::Fill))
        .on_press(Message::SwatchClicked(recipe))
        .padding([2, 6])
        .width(Length::Fill)
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: if hovered {
                    Some(tokens::HOVER_OVERLAY_SUBTLE.into())
                } else {
                    None
                },
                text_color: pal.background.base.text,
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    container(iced::widget::column![canvas_inner, label].spacing(2))
        .padding(3)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            let pal = theme.palette();
            container::Style {
                background: Some(
                    tokens::chrome_band(
                        pal.background.base.text,
                        tokens::CHROME_BAND_FAINT,
                    )
                    .into(),
                ),
                border: iced::Border {
                    radius: tokens::RADIUS_BUTTON.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the Activity Log bucket — Navigator's right-rail
/// "what just happened" stream. Per
/// [`iced_composition_skeleton_spec.md` §6.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
///
/// Slice 27: snapshots the host's bounded `RecordingObserver`
/// (capacity `ACTIVITY_LOG_CAPACITY = 100`) and renders one row per
/// recorded event in most-recent-first order. Empty buffer shows a
/// "no activity yet" placeholder. Subsequent slices can add
/// click-to-navigate (e.g., a row for `OpenNodeDispatched` could
/// re-focus that node) and visual filtering.
pub(super) fn render_activity_log_bucket(app: &IcedApp) -> Element<'_, Message> {
    let header: Element<'_, Message> =
        text("Activity Log").size(11).width(Length::Fill).into();

    let events = app.activity_log_recorder.snapshot();
    if events.is_empty() {
        return scrollable(
            iced::widget::column![header, text("  — no activity yet").size(11)].spacing(4),
        )
        .height(Length::FillPortion(1))
        .into();
    }

    // Render most-recent first. The recorder appends in observation
    // order, so reverse for display.
    let rows: Vec<Element<'_, Message>> = events
        .into_iter()
        .rev()
        .map(activity_log_row)
        .collect();

    let mut col = iced::widget::column![header];
    for row in rows {
        col = col.push(row);
    }

    scrollable(col.spacing(2).padding([2, 0]))
        .height(Length::FillPortion(1))
        .into()
}

/// One row in the Activity Log. Renders a single line of text
/// describing the event; click handlers land in a future slice.
pub(super) fn activity_log_row<'a>(
    event: graphshell_core::ux_observability::UxEvent,
) -> Element<'a, Message> {
    text(format_ux_event(&event))
        .size(11)
        .width(Length::Fill)
        .into()
}

/// Convert a `UxEvent` into a concise human-readable summary line
/// for the Activity Log. Surface variants render as
/// `"opened: Command Palette"` / `"dismissed: Node Finder (cancelled)"`;
/// dispatches render as `"action: graph:toggle_physics"` /
/// `"open node: n7"`.
pub(super) fn format_ux_event(event: &graphshell_core::ux_observability::UxEvent) -> String {
    use graphshell_core::ux_observability::{DismissReason, SurfaceId, UxEvent};
    fn surface_label(s: SurfaceId) -> &'static str {
        match s {
            SurfaceId::Omnibar => "Omnibar",
            SurfaceId::CommandPalette => "Command Palette",
            SurfaceId::NodeFinder => "Node Finder",
            SurfaceId::ContextMenu => "Context Menu",
            SurfaceId::ConfirmDialog => "Confirm Dialog",
            SurfaceId::NodeCreate => "Create Node",
            SurfaceId::FrameRename => "Rename Frame",
            SurfaceId::StatusBar => "Status Bar",
            SurfaceId::TreeSpine => "Tree Spine",
            SurfaceId::NavigatorHost => "Navigator",
            SurfaceId::TilePane => "Tile Pane",
            SurfaceId::CanvasPane => "Canvas Pane",
            SurfaceId::BaseLayer => "Base Layer",
        }
    }
    fn reason_label(r: DismissReason) -> &'static str {
        match r {
            DismissReason::Confirmed => "confirmed",
            DismissReason::Cancelled => "cancelled",
            DismissReason::Superseded => "superseded",
            DismissReason::Programmatic => "programmatic",
        }
    }
    match event {
        UxEvent::SurfaceOpened { surface } => {
            format!("opened: {}", surface_label(*surface))
        }
        UxEvent::SurfaceDismissed { surface, reason } => format!(
            "dismissed: {} ({})",
            surface_label(*surface),
            reason_label(*reason)
        ),
        UxEvent::ActionDispatched { action_id, target } => match target {
            Some(node_key) => {
                format!("action: {} → n{}", action_id.key(), node_key.index())
            }
            None => format!("action: {}", action_id.key()),
        },
        UxEvent::OpenNodeDispatched { node_key } => {
            format!("open node: n{}", node_key.index())
        }
    }
}

/// One row in the Tree Spine list. Click dispatches an OpenNode
/// intent against the row's NodeKey.
pub(super) fn tree_spine_row<'a>(
    node_key: graphshell_core::graph::NodeKey,
    label: String,
) -> Element<'a, Message> {
    button(text(label).size(11).width(Length::Fill))
        .on_press(Message::TreeSpineNodeClicked(node_key))
        .padding([2, 6])
        .width(Length::Fill)
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: if hovered {
                    Some(tokens::HOVER_OVERLAY_SUBTLE.into())
                } else {
                    None
                },
                text_color: pal.background.base.text,
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the StatusBar slot. Per
/// [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// ambient system status, process indicators, background task count.
///
/// Slice 19 wires four indicators sourced from runtime state:
/// - **status dot** — green "ready" pulse (will animate on activity
///   in a later slice with `cosmic-time`)
/// - **actions** — `runtime.dispatched_action_count` (cumulative
///   `HostIntent::Action` dispatches since runtime start)
/// - **pending** — `host.pending_host_intents.len()` (queued intents
///   awaiting the next tick drain)
/// - **focused** — `runtime.focused_node_hint` (rendered as the
///   underlying NodeKey index, or "—" when no node is focused)
pub(super) fn render_status_bar(app: &IcedApp) -> Element<'_, Message> {
    let dispatched = app.host.runtime.dispatched_action_count;
    let pending = app.host.pending_host_intents.len();
    let focused_label = app
        .host
        .runtime
        .focused_node_hint
        .map(|k| format!("n{}", k.index()))
        .unwrap_or_else(|| "—".to_string());

    let dot = text("●").size(11).style(|theme: &iced::Theme| {
        let pal = theme.palette();
        iced::widget::text::Style {
            color: Some(pal.success.base.color),
        }
    });
    let ready = text("ready").size(11);
    let actions = text(format!("actions: {dispatched}")).size(11);
    let pending_text = text(format!("pending: {pending}")).size(11);
    let focused = text(format!("focused: {focused_label}")).size(11);

    container(
        iced::widget::row![
            dot,
            ready,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            actions,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            pending_text,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            focused,
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .height(Length::Fixed(20.0))
    .style(|theme: &iced::Theme| {
        let pal = theme.palette();
        container::Style {
            background: Some(
                tokens::chrome_band(pal.background.base.text, tokens::CHROME_BAND_BASE)
                    .into(),
            ),
            ..Default::default()
        }
    })
    .into()
}

/// Render the host's toast queue as a stack of severity-prefixed rows.
pub(super) fn render_toast_stack(
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
