//! Pane render functions extracted from `view/mod.rs` —
//! Slice 45 / Phase D continuation. Covers the FrameSplitTree slot
//! and per-Pane body renders: `pane_grid` driver, the
//! Tile/Canvas/BaseLayer dispatch, the tile-tab + settings-pane
//! body composition, and the `tiles_for_pane` data builder.

use super::*;

pub(crate) fn render_frame_split_tree(app: &IcedApp) -> Element<'_, Message> {
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
pub(crate) fn render_pane_body<'a>(app: &'a IcedApp, meta: &PaneMeta) -> Element<'a, Message> {
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
pub(crate) fn render_settings_pane(app: &IcedApp) -> Element<'_, Message> {
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
pub(crate) fn render_tile_pane_body<'a>(app: &'a IcedApp, meta: &PaneMeta) -> Element<'a, Message> {
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
pub(crate) fn tiles_for_pane(
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
pub(crate) fn render_canvas_base_layer(app: &IcedApp) -> Element<'_, Message> {
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
