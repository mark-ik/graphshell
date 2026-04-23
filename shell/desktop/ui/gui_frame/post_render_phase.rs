/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::render_backend::UiRenderBackendHandle;
use crate::shell::desktop::ui::dialog::DialogCommand;
use crate::shell::desktop::ui::gui_state::PendingWebviewContextSurfaceRequest;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::ui::gui_state::RuntimeFocusInspector;
use crate::shell::desktop::ui::workbench_host::{WorkbenchChromeProjection, WorkbenchLayerState};

const WORKBENCH_AREA_DEFAULT_FRACTION: f32 = 0.46;
const WORKBENCH_AREA_MAX_FRACTION: f32 = 0.82;
const WORKBENCH_AREA_MIN_WIDTH: f32 = 360.0;
const WORKBENCH_OVERLAY_WIDTH_FRACTION: f32 = 0.72;
const WORKBENCH_OVERLAY_HEIGHT_FRACTION: f32 = 0.82;
const WORKBENCH_OVERLAY_MIN_WIDTH: f32 = 520.0;
const WORKBENCH_OVERLAY_MIN_HEIGHT: f32 = 360.0;
const WORKBENCH_OVERLAY_MARGIN: f32 = 20.0;
const WORKBENCH_OVERLAY_TITLE_BAR_HEIGHT: f32 = 30.0;
const WORKBENCH_OVERLAY_DRAG_HANDLE_MAX_WIDTH: f32 = 180.0;
const WORKBENCH_OVERLAY_RESIZE_GRIP: f32 = 18.0;

fn workbench_area_max_width(available_rect: egui::Rect) -> f32 {
    (available_rect.width() * WORKBENCH_AREA_MAX_FRACTION).max(WORKBENCH_AREA_MIN_WIDTH)
}

fn workbench_area_default_width(available_rect: egui::Rect) -> f32 {
    (available_rect.width() * WORKBENCH_AREA_DEFAULT_FRACTION).clamp(
        WORKBENCH_AREA_MIN_WIDTH,
        workbench_area_max_width(available_rect),
    )
}

fn workbench_overlay_max_size(available_rect: egui::Rect) -> egui::Vec2 {
    egui::vec2(
        (available_rect.width() - WORKBENCH_OVERLAY_MARGIN * 2.0).max(1.0),
        (available_rect.height() - WORKBENCH_OVERLAY_MARGIN * 2.0).max(1.0),
    )
}

fn workbench_overlay_default_size(available_rect: egui::Rect) -> egui::Vec2 {
    let max_size = workbench_overlay_max_size(available_rect);
    egui::vec2(
        (available_rect.width() * WORKBENCH_OVERLAY_WIDTH_FRACTION)
            .clamp(WORKBENCH_OVERLAY_MIN_WIDTH.min(max_size.x), max_size.x),
        (available_rect.height() * WORKBENCH_OVERLAY_HEIGHT_FRACTION)
            .clamp(WORKBENCH_OVERLAY_MIN_HEIGHT.min(max_size.y), max_size.y),
    )
}

fn clamp_workbench_overlay_size(available_rect: egui::Rect, size: egui::Vec2) -> egui::Vec2 {
    let max_size = workbench_overlay_max_size(available_rect);
    egui::vec2(
        size.x
            .clamp(WORKBENCH_OVERLAY_MIN_WIDTH.min(max_size.x), max_size.x),
        size.y
            .clamp(WORKBENCH_OVERLAY_MIN_HEIGHT.min(max_size.y), max_size.y),
    )
}

fn workbench_overlay_default_pos(available_rect: egui::Rect, size: egui::Vec2) -> egui::Pos2 {
    egui::pos2(
        available_rect.right() - size.x - WORKBENCH_OVERLAY_MARGIN,
        available_rect.center().y - size.y * 0.5,
    )
}

fn clamp_workbench_overlay_pos(
    available_rect: egui::Rect,
    size: egui::Vec2,
    pos: egui::Pos2,
) -> egui::Pos2 {
    let min_x = available_rect.left() + WORKBENCH_OVERLAY_MARGIN;
    let min_y = available_rect.top() + WORKBENCH_OVERLAY_MARGIN;
    let max_x = (available_rect.right() - size.x - WORKBENCH_OVERLAY_MARGIN).max(min_x);
    let max_y = (available_rect.bottom() - size.y - WORKBENCH_OVERLAY_MARGIN).max(min_y);
    egui::pos2(pos.x.clamp(min_x, max_x), pos.y.clamp(min_y, max_y))
}

fn workbench_overlay_rect(
    available_rect: egui::Rect,
    stored_pos: Option<egui::Pos2>,
    stored_size: Option<egui::Vec2>,
) -> egui::Rect {
    let size = clamp_workbench_overlay_size(
        available_rect,
        stored_size.unwrap_or_else(|| workbench_overlay_default_size(available_rect)),
    );
    let pos = clamp_workbench_overlay_pos(
        available_rect,
        size,
        stored_pos.unwrap_or_else(|| workbench_overlay_default_pos(available_rect, size)),
    );
    egui::Rect::from_min_size(pos, size)
}

fn open_preferred_context_command_surface_for_webview_target(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    webview_id: WebViewId,
    anchor: [f32; 2],
) -> bool {
    let Some(node_key) = graph_app.get_node_for_webview(
        crate::shell::desktop::lifecycle::webview_status_sync::renderer_id_from_servo(webview_id),
    ) else {
        return false;
    };

    match graph_app.context_command_surface_preference() {
        crate::app::ContextCommandSurfacePreference::RadialPalette => {
            graph_app.set_pending_node_context_target(Some(node_key));
            if graph_app
                .pending_transient_surface_return_target()
                .is_none()
            {
                graph_app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Node(node_key),
                ));
            }
            ctx.data_mut(|d| {
                d.insert_persisted(
                    egui::Id::new("radial_menu_center"),
                    egui::pos2(anchor[0], anchor[1]),
                );
            });
            graph_app.open_radial_menu();
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            graph_app.set_pending_node_context_target(Some(node_key));
            if graph_app.pending_command_surface_return_target().is_none() {
                graph_app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Node(node_key),
                ));
            }
            graph_app.set_context_palette_anchor(Some(anchor));
            graph_app.open_context_palette();
        }
    }
    true
}

pub(crate) struct PostRenderPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) root_ui: &'a mut egui::Ui,
    pub(crate) ui_render_backend: &'a mut UiRenderBackendHandle,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) bookmark_import_dialog: &'a mut Option<BookmarkImportDialogState>,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) headed_window: &'a HeadedWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) graph_tree: &'a mut graph_tree::GraphTree<NodeKey>,
    pub(crate) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    pub(crate) viewer_surface_host: &'a mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(crate) graph_search_matches: &'a [NodeKey],
    pub(crate) graph_search_active_match_index: Option<usize>,
    pub(crate) graph_search_filter_mode: bool,
    pub(crate) search_query_active: bool,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) command_surface_telemetry:
        &'a crate::shell::desktop::ui::command_surface_telemetry::CommandSurfaceTelemetry,
    /// Focus mutation bundle carried in from the frame pipeline. Replaces
    /// the prior per-field `focused_node_hint` / `focus_ring_node_key` /
    /// `focus_ring_started_at` / `focus_ring_duration` fields. See
    /// [`FocusAuthorityMut`](crate::shell::desktop::ui::gui_state::FocusAuthorityMut).
    pub(crate) focus: crate::shell::desktop::ui::gui_state::FocusAuthorityMut<'a>,
    /// Command-palette mutation bundle. Replaces the pre-M4 pattern of
    /// stashing palette state inside `egui::Context::data_mut(...)`
    /// persistent storage; the widget now reads/writes the runtime-
    /// owned `CommandPaletteSession` through this bundle.
    pub(crate) command_authority:
        crate::shell::desktop::ui::gui_state::CommandAuthorityMut<'a>,
    pub(crate) pending_webview_context_surface_requests:
        &'a mut Vec<PendingWebviewContextSurfaceRequest>,
    pub(crate) toasts: &'a mut egui_notify::Toasts,
    pub(crate) control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub(crate) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
    #[cfg(feature = "diagnostics")]
    pub(crate) runtime_focus_inspector: Option<RuntimeFocusInspector>,
}

pub(crate) fn run_post_render_phase<FActive>(
    args: PostRenderPhaseArgs<'_>,
    active_graph_search_match: FActive,
) where
    FActive: Fn(&[NodeKey], Option<usize>) -> Option<NodeKey>,
{
    let PostRenderPhaseArgs {
        ctx,
        root_ui,
        ui_render_backend,
        graph_app,
        bookmark_import_dialog,
        window,
        headed_window,
        tiles_tree,
        graph_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        favicon_textures,
        toolbar_height,
        graph_search_matches,
        graph_search_active_match_index,
        graph_search_filter_mode,
        search_query_active,
        app_state,
        rendering_context,
        window_rendering_context,
        responsive_webviews,
        webview_creation_backpressure,
        mut focus,
        mut command_authority,
        pending_webview_context_surface_requests,
        toasts,
        control_panel,
        command_surface_telemetry,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
        #[cfg(feature = "diagnostics")]
        runtime_focus_inspector,
    } = args;

    #[cfg(debug_assertions)]
    {
        for violation in tile_invariants::collect_tile_invariant_violations(
            tiles_tree,
            graph_app,
            viewer_surfaces,
        ) {
            warn!("{violation}");
        }
    }

    let preview_mode_active = history_preview_mode_active(graph_app);

    let mut bookmark_import_requested = false;
    while graph_app.take_pending_import_bookmarks_from_file() {
        bookmark_import_requested = true;
    }
    if bookmark_import_requested && bookmark_import_dialog.is_none() {
        *bookmark_import_dialog = Some(BookmarkImportDialogState::new());
    }

    match bookmark_import_dialog
        .as_mut()
        .map(|dialog| dialog.update(ctx))
    {
        Some(BookmarkImportDialogEvent::Continue) | None => {}
        Some(BookmarkImportDialogEvent::Cancelled) => {
            *bookmark_import_dialog = None;
        }
        Some(BookmarkImportDialogEvent::Picked(path)) => {
            *bookmark_import_dialog = None;
            import_bookmarks_from_path(graph_app, toasts, &path);
        }
    }

    *toolbar_height = Length::new(ctx.content_rect().min.y);
    if !preview_mode_active {
        graph_app.check_periodic_snapshot();
    }

    for PendingWebviewContextSurfaceRequest { webview_id, anchor } in
        pending_webview_context_surface_requests.drain(..)
    {
        let Some(servo_webview_id) =
            crate::shell::desktop::lifecycle::webview_status_sync::servo_webview_id_from_viewer_instance(
                &webview_id,
            )
        else {
            // Non-Servo provider (Wry, iced_webview, MiddleNet) — the
            // Servo-specific context-surface dispatch below can't
            // service it. Drop the request silently; the originating
            // provider is responsible for offering its own context
            // menu affordance.
            continue;
        };
        let _ = open_preferred_context_command_surface_for_webview_target(
            ctx, graph_app, servo_webview_id, anchor,
        );
    }

    let focused_dialog_webview = if focus.graph_surface_focused() {
        None
    } else {
        window.explicit_dialog_webview_id()
    };
    let dialog_commands = std::cell::RefCell::new(Vec::new());
    headed_window.for_each_active_dialog(
        window,
        focused_dialog_webview,
        *toolbar_height,
        |dialog| {
            let result = dialog.update(ctx);
            if let Some(command) = result.command {
                dialog_commands.borrow_mut().push(command);
            }
            result.keep_open
        },
    );
    for command in dialog_commands.into_inner() {
        match command {
            DialogCommand::ClipElement {
                webview_id,
                element_rect,
            } => headed_window.request_clip_element(window, webview_id, element_rect),
            DialogCommand::InspectPageElements { webview_id } => {
                headed_window.request_page_inspector_candidates(window, webview_id)
            }
        }
    }

    let mut post_render_intents = Vec::new();
    let search_matches: HashSet<NodeKey> = graph_search_matches.iter().copied().collect();
    let active_search_match =
        active_graph_search_match(graph_search_matches, graph_search_active_match_index);
    let layer_state =
        WorkbenchChromeProjection::from_tree(graph_app, tiles_tree, window.focused_pane())
            .layer_state;

    if matches!(
        layer_state,
        WorkbenchLayerState::WorkbenchActive | WorkbenchLayerState::WorkbenchPinned
    ) {
        let available_rect = ctx.content_rect();
        let panel_bg = crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
            graph_app.default_registry_theme_id(),
        )
        .tokens
        .workbench_panel_background;
        let focus_arg = focus.reborrow();
        egui::Panel::right("workbench_area")
            .resizable(true)
            .default_size(workbench_area_default_width(available_rect))
            .min_size(WORKBENCH_AREA_MIN_WIDTH)
            .max_size(workbench_area_max_width(available_rect))
            .frame(egui::Frame::new().fill(panel_bg))
            .show_inside(root_ui, |ui| {
                post_render_intents.extend(tile_render_pass::run_tile_render_pass_in_ui(
                    ui,
                    TileRenderPassArgs {
                        ctx,
                        ui_render_backend,
                        graph_app,
                        window,
                        tiles_tree,
                        graph_tree,
                        viewer_surfaces,
                        viewer_surface_host,
                        tile_favicon_textures,
                        graph_search_matches: &search_matches,
                        active_search_match,
                        graph_search_filter_mode,
                        search_query_active,
                        app_state,
                        rendering_context,
                        window_rendering_context,
                        responsive_webviews,
                        webview_creation_backpressure,
                        focus: focus_arg,
                        suppress_runtime_side_effects: preview_mode_active,
                        control_panel,
                        command_surface_telemetry,
                        #[cfg(feature = "diagnostics")]
                        diagnostics_state,
                        #[cfg(feature = "diagnostics")]
                        runtime_focus_inspector: runtime_focus_inspector.clone(),
                    },
                ));
            });
    }

    let central_panel_bg = crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
        graph_app.default_registry_theme_id(),
    )
    .tokens
    .workbench_panel_background;
    let graph_surface_focused = focus.graph_surface_focused();
    let focus_arg = focus.reborrow();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(central_panel_bg))
        .show_inside(root_ui, |ui| {
            if matches!(layer_state, WorkbenchLayerState::WorkbenchOnly) {
                post_render_intents.extend(tile_render_pass::run_tile_render_pass_in_ui(
                    ui,
                    TileRenderPassArgs {
                        ctx,
                        ui_render_backend,
                        graph_app,
                        window,
                        tiles_tree,
                        graph_tree,
                        viewer_surfaces,
                        viewer_surface_host,
                        tile_favicon_textures,
                        graph_search_matches: &search_matches,
                        active_search_match,
                        graph_search_filter_mode,
                        search_query_active,
                        app_state,
                        rendering_context,
                        window_rendering_context,
                        responsive_webviews,
                        webview_creation_backpressure,
                        focus: focus_arg,
                        suppress_runtime_side_effects: preview_mode_active,
                        control_panel,
                        command_surface_telemetry,
                        #[cfg(feature = "diagnostics")]
                        diagnostics_state,
                        #[cfg(feature = "diagnostics")]
                        runtime_focus_inspector: runtime_focus_inspector.clone(),
                    },
                ));
            } else {
                post_render_intents.extend(tile_render_pass::render_primary_graph_in_ui(
                    ui,
                    graph_app,
                    tiles_tree,
                    graph_tree,
                    &search_matches,
                    active_search_match,
                    graph_search_filter_mode,
                    search_query_active,
                    graph_surface_focused,
                ));
            }
        });

    if matches!(layer_state, WorkbenchLayerState::WorkbenchOverlayActive) {
        let available_rect = ctx.content_rect();
        let overlay_pos_id = egui::Id::new("workbench_overlay_pos");
        let overlay_size_id = egui::Id::new("workbench_overlay_size");
        let overlay_drag_origin_id = egui::Id::new("workbench_overlay_drag_origin");
        let overlay_resize_origin_id = egui::Id::new("workbench_overlay_resize_origin");
        let stored_overlay_pos = ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(overlay_pos_id));
        let stored_overlay_size = ctx.data_mut(|d| d.get_persisted::<egui::Vec2>(overlay_size_id));
        let overlay_default_size = workbench_overlay_default_size(available_rect);
        let overlay_default_pos =
            workbench_overlay_default_pos(available_rect, overlay_default_size);
        let overlay_default_rect =
            egui::Rect::from_min_size(overlay_default_pos, overlay_default_size);
        let overlay_rect =
            workbench_overlay_rect(available_rect, stored_overlay_pos, stored_overlay_size);
        let backdrop_layer = egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("workbench_overlay_backdrop"),
        );
        ctx.layer_painter(backdrop_layer).rect_filled(
            available_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(6, 8, 12, 168),
        );

        egui::Area::new(egui::Id::new("workbench_overlay_backdrop_hit"))
            .order(egui::Order::Foreground)
            .fixed_pos(available_rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                let response = ui.allocate_rect(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, available_rect.size()),
                    egui::Sense::click(),
                );
                let clicked_outside_overlay = response.clicked()
                    && response
                        .interact_pointer_pos()
                        .is_some_and(|pos| !overlay_rect.contains(pos));
                if clicked_outside_overlay {
                    graph_app.enqueue_workbench_intent(
                        crate::app::WorkbenchIntent::SetWorkbenchOverlayVisible { visible: false },
                    );
                }
            });

        let mut next_overlay_pos = overlay_rect.min;
        let mut next_overlay_size = overlay_rect.size();
        let mut reset_overlay_layout = false;
        egui::Area::new(egui::Id::new("workbench_overlay_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(overlay_rect.min)
            .movable(false)
            .show(ctx, |ui| {
                ui.set_min_size(overlay_rect.size());
                let frame_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, overlay_rect.size());
                let drag_handle_width = frame_rect
                    .width()
                    .min(WORKBENCH_OVERLAY_DRAG_HANDLE_MAX_WIDTH)
                    .max(120.0);
                let drag_rect = egui::Rect::from_min_size(
                    frame_rect.min,
                    egui::vec2(drag_handle_width, WORKBENCH_OVERLAY_TITLE_BAR_HEIGHT),
                );
                let drag_response = ui.interact(
                    drag_rect,
                    egui::Id::new("workbench_overlay_drag_handle"),
                    egui::Sense::click_and_drag(),
                );
                if drag_response.drag_started() {
                    ctx.data_mut(|d| d.insert_temp(overlay_drag_origin_id, overlay_rect.min));
                }
                if drag_response.dragged() {
                    let drag_origin = ctx
                        .data_mut(|d| d.get_temp::<egui::Pos2>(overlay_drag_origin_id))
                        .unwrap_or(overlay_rect.min);
                    next_overlay_pos = drag_origin + drag_response.drag_delta();
                    ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::Grabbing);
                } else if drag_response.hovered() {
                    ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::Grab);
                }

                let resize_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        (frame_rect.max.x - WORKBENCH_OVERLAY_RESIZE_GRIP).max(frame_rect.min.x),
                        (frame_rect.max.y - WORKBENCH_OVERLAY_RESIZE_GRIP).max(frame_rect.min.y),
                    ),
                    egui::vec2(WORKBENCH_OVERLAY_RESIZE_GRIP, WORKBENCH_OVERLAY_RESIZE_GRIP),
                );
                let resize_response = ui.interact(
                    resize_rect,
                    egui::Id::new("workbench_overlay_resize_grip"),
                    egui::Sense::click_and_drag(),
                );
                if resize_response.drag_started() {
                    ctx.data_mut(|d| d.insert_temp(overlay_resize_origin_id, overlay_rect.size()));
                }
                if resize_response.dragged() {
                    let resize_origin = ctx
                        .data_mut(|d| d.get_temp::<egui::Vec2>(overlay_resize_origin_id))
                        .unwrap_or(overlay_rect.size());
                    next_overlay_size = resize_origin + resize_response.drag_delta();
                    ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::ResizeNwSe);
                } else if resize_response.hovered() {
                    ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::ResizeNwSe);
                }

                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 20, 28))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(62, 68, 82)))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_min_size(overlay_rect.size());
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Workbench overlay").strong());
                            ui.label(
                                egui::RichText::new("Drag header • resize corner")
                                    .small()
                                    .color(egui::Color32::from_rgb(146, 152, 168)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Close overlay").clicked() {
                                        graph_app.enqueue_workbench_intent(
                                            crate::app::WorkbenchIntent::SetWorkbenchOverlayVisible {
                                                visible: false,
                                            },
                                        );
                                    }
                                    if ui.small_button("Reset layout").clicked() {
                                        reset_overlay_layout = true;
                                    }
                                },
                            );
                        });
                        ui.separator();
                        post_render_intents.extend(tile_render_pass::run_tile_render_pass_in_ui(
                            ui,
                            TileRenderPassArgs {
                                ctx,
                                ui_render_backend,
                                graph_app,
                                window,
                                tiles_tree,
                                graph_tree,
                                viewer_surfaces,
                                viewer_surface_host,
                                tile_favicon_textures,
                                graph_search_matches: &search_matches,
                                active_search_match,
                                graph_search_filter_mode,
                                search_query_active,
                                app_state,
                                rendering_context,
                                window_rendering_context,
                                responsive_webviews,
                                webview_creation_backpressure,
                                focus: focus.reborrow(),
                                suppress_runtime_side_effects: preview_mode_active,
                                control_panel,
                                command_surface_telemetry,
                                #[cfg(feature = "diagnostics")]
                                diagnostics_state,
                                #[cfg(feature = "diagnostics")]
                                runtime_focus_inspector: runtime_focus_inspector.clone(),
                            },
                        ));
                    });

                let grip_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(108, 116, 132));
                ui.painter().line_segment(
                    [
                        egui::pos2(resize_rect.right() - 11.0, resize_rect.bottom()),
                        egui::pos2(resize_rect.right(), resize_rect.bottom() - 11.0),
                    ],
                    grip_stroke,
                );
                ui.painter().line_segment(
                    [
                        egui::pos2(resize_rect.right() - 7.0, resize_rect.bottom()),
                        egui::pos2(resize_rect.right(), resize_rect.bottom() - 7.0),
                    ],
                    grip_stroke,
                );
            });

        if reset_overlay_layout {
            next_overlay_pos = overlay_default_rect.min;
            next_overlay_size = overlay_default_rect.size();
        }
        let clamped_overlay_size = clamp_workbench_overlay_size(available_rect, next_overlay_size);
        let clamped_overlay_pos =
            clamp_workbench_overlay_pos(available_rect, clamped_overlay_size, next_overlay_pos);
        ctx.data_mut(|d| {
            d.insert_persisted(overlay_pos_id, clamped_overlay_pos);
            d.insert_persisted(overlay_size_id, clamped_overlay_size);
        });
    }

    apply_intents_if_any(graph_app, tiles_tree, &mut post_render_intents);

    render::render_help_panel(ctx, graph_app);
    render::render_scene_overlay_panel(ctx, graph_app);
    render::render_settings_overlay_panel(ctx, graph_app, Some(control_panel));
    render::render_clip_inspector_panel(ctx, graph_app);
    if let Some(webview_id) = graph_app
        .workspace
        .graph_runtime
        .pending_clip_inspector_highlight_clear
        .take()
    {
        headed_window.sync_clip_inspector_highlight(window, webview_id, None);
    }
    if let Some(state) = graph_app
        .workspace
        .graph_runtime
        .clip_inspector_state
        .as_ref()
        && state.highlight_dirty
    {
        headed_window.sync_clip_inspector_highlight(
            window,
            state.webview_id,
            state
                .pointer_stack
                .get(state.pointer_stack_index)
                .and_then(|capture| capture.dom_path.as_deref()),
        );
        graph_app.clear_clip_inspector_highlight_dirty();
    }
    let active_pane_first = graph_app
        .workspace
        .graph_runtime
        .active_pane_rects
        .first()
        .map(|(pane_id, node_key, _)| (*pane_id, *node_key));
    let active_node_pane_key = active_pane_first.map(|(_, nk)| nk);
    let active_node_pane_id = active_pane_first.map(|(pid, _)| pid);
    let focused_pane_node = nav_targeting::chrome_projection_node(graph_app, window)
        .or_else(|| {
            focused_dialog_webview.and_then(|webview_id| {
                graph_app.get_node_for_webview(
                    crate::shell::desktop::lifecycle::webview_status_sync::renderer_id_from_servo(
                        webview_id,
                    ),
                )
            })
        })
        .or(active_node_pane_key);
    render::render_command_palette_panel(
        ctx,
        graph_app,
        command_authority.reborrow(),
        graph_app.workspace.graph_runtime.hovered_graph_node,
        focused_pane_node,
        active_node_pane_id,
    );
    render::render_radial_command_menu(
        ctx,
        graph_app,
        graph_app.workspace.graph_runtime.hovered_graph_node,
        focused_pane_node,
        active_node_pane_id,
    );
    crate::shell::desktop::ui::tag_panel::render_tag_panel(
        ctx,
        graph_app,
        tiles_tree,
        focus.graph_surface_focused(),
        focus.hint(),
    );
    crate::shell::desktop::ui::overview_plane::render_overview_plane(
        ctx,
        graph_app,
        tiles_tree,
        window.focused_pane(),
    );
    if !preview_mode_active && let Some(target_dir) = graph_app.take_pending_switch_data_dir() {
        match persistence_ops::switch_persistence_store(
            graph_app,
            window,
            tiles_tree,
            viewer_surfaces,
            viewer_surface_host,
            tile_favicon_textures,
            favicon_textures,
            &mut post_render_intents,
            target_dir.clone(),
        ) {
            Ok(()) => toasts.success(format!(
                "Switched graph data directory to {}",
                target_dir.display()
            )),
            Err(e) => toasts.error(format!("Failed to switch data directory: {e}")),
        };
    }
    apply_intents_if_any(graph_app, tiles_tree, &mut post_render_intents);

    let open_settings_tool_pane = render::render_choose_frame_picker(ctx, graph_app)
        || render::render_unsaved_frame_prompt(ctx, graph_app);
    if open_settings_tool_pane {
        tile_view_ops::open_or_focus_tool_pane(tiles_tree, ToolPaneState::Settings);
    }

    if !preview_mode_active {
        pending_actions::run_post_render_pending_actions(
            graph_app,
            window,
            tiles_tree,
            viewer_surfaces,
            viewer_surface_host,
            tile_favicon_textures,
            webview_creation_backpressure,
            focus.focused_node_hint,
        );
    }

    while let Some((key, url)) = graph_app.take_pending_protocol_probe() {
        control_panel.handle_protocol_probe_request(key, url);
    }
}

fn import_bookmarks_from_path(
    graph_app: &mut GraphBrowserApp,
    toasts: &mut egui_notify::Toasts,
    path: &std::path::Path,
) {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            toasts.error(format!(
                "Failed to read bookmark file {}: {error}",
                path.display()
            ));
            return;
        }
    };

    let run = bookmark_import_run_for_path(path);
    let batch = match crate::services::import::parse_bookmark_file_to_batch(&contents, run) {
        Ok(batch) => batch,
        Err(error) => {
            toasts.error(format!(
                "Failed to import bookmark file {}: {error}",
                path.display()
            ));
            return;
        }
    };
    let imported_count = batch.items.len();
    let label = batch.run.user_visible_label.clone();
    graph_app.apply_browser_import_batch(&batch, None);
    toasts.success(format!(
        "Imported {imported_count} bookmark item(s) from {label}"
    ));
}

fn bookmark_import_run_for_path(
    path: &std::path::Path,
) -> crate::services::import::BrowserImportRun {
    let observed_at_unix_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("bookmark-file");
    let stable_token = sanitize_bookmark_import_token(file_name);

    crate::services::import::BrowserImportRun {
        import_id: format!("import-run:bookmark-file:{stable_token}:{observed_at_unix_secs}"),
        source: crate::services::import::BrowserImportSource {
            browser_family: crate::services::import::BrowserFamily::Other(
                "bookmark-file".to_string(),
            ),
            profile_hint: None,
            source_kind: crate::services::import::BrowserImportSourceKind::BookmarkFile,
            stable_source_id: Some(format!("bookmark-file:{stable_token}")),
        },
        mode: crate::services::import::BrowserImportMode::OneShotFile,
        observed_at_unix_secs,
        user_visible_label: file_name.to_string(),
    }
}

fn sanitize_bookmark_import_token(value: &str) -> String {
    let token = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let token = token.trim_matches('-');
    if token.is_empty() {
        "bookmark-file".to_string()
    } else {
        token.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WORKBENCH_AREA_MIN_WIDTH, WORKBENCH_OVERLAY_MARGIN,
        open_preferred_context_command_surface_for_webview_target, workbench_area_default_width,
        workbench_area_max_width, workbench_overlay_default_size, workbench_overlay_rect,
    };
    use crate::app::{ContextCommandSurfacePreference, GraphBrowserApp, ToolSurfaceReturnTarget};
    use servo::WebViewId;

    fn test_webview_id() -> WebViewId {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(91));
                cell.set(true);
            }
        });
        WebViewId::new(base::id::PainterId::next())
    }

    #[test]
    fn mapped_webview_target_opens_context_palette_when_preferred() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://context.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(
            crate::shell::desktop::lifecycle::webview_status_sync::renderer_id_from_servo(webview_id),
            node,
        );

        app.set_context_command_surface_preference(ContextCommandSurfacePreference::ContextPalette);
        let ctx = egui::Context::default();

        assert!(open_preferred_context_command_surface_for_webview_target(
            &ctx,
            &mut app,
            webview_id,
            [12.0, 24.0]
        ));
        assert_eq!(app.pending_node_context_target(), Some(node));
        assert_eq!(
            app.pending_command_surface_return_target(),
            Some(ToolSurfaceReturnTarget::Node(node))
        );
        assert_eq!(
            app.workspace.chrome_ui.context_palette_anchor,
            Some([12.0, 24.0])
        );
        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(!app.workspace.chrome_ui.show_command_palette);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
    }

    #[test]
    fn mapped_webview_target_opens_radial_menu_when_preferred() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://context.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(
            crate::shell::desktop::lifecycle::webview_status_sync::renderer_id_from_servo(webview_id),
            node,
        );
        app.set_context_command_surface_preference(ContextCommandSurfacePreference::RadialPalette);
        let ctx = egui::Context::default();

        assert!(open_preferred_context_command_surface_for_webview_target(
            &ctx,
            &mut app,
            webview_id,
            [20.0, 36.0]
        ));
        assert_eq!(app.pending_node_context_target(), Some(node));
        assert_eq!(
            app.pending_transient_surface_return_target(),
            Some(ToolSurfaceReturnTarget::Node(node))
        );
        assert!(app.workspace.chrome_ui.show_radial_menu);
        assert!(!app.workspace.chrome_ui.show_context_palette);
        let center =
            ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(egui::Id::new("radial_menu_center")));
        assert_eq!(center, Some(egui::pos2(20.0, 36.0)));
    }

    #[test]
    fn webview_context_command_surface_ignores_unmapped_webview() {
        let mut app = GraphBrowserApp::new_for_testing();
        let webview_id = test_webview_id();
        let ctx = egui::Context::default();

        assert!(!open_preferred_context_command_surface_for_webview_target(
            &ctx,
            &mut app,
            webview_id,
            [4.0, 8.0]
        ));
        assert_eq!(app.pending_node_context_target(), None);
        assert_eq!(app.pending_command_surface_return_target(), None);
        assert_eq!(app.pending_transient_surface_return_target(), None);
        assert!(!app.workspace.chrome_ui.show_context_palette);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
    }

    #[test]
    fn workbench_area_default_width_uses_large_resizable_footprint() {
        let available_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1600.0, 900.0));

        let default_width = workbench_area_default_width(available_rect);
        let max_width = workbench_area_max_width(available_rect);

        assert!(default_width >= WORKBENCH_AREA_MIN_WIDTH);
        assert!(default_width > 600.0);
        assert!(default_width <= max_width);
    }

    #[test]
    fn workbench_overlay_rect_clamps_saved_layout_into_visible_bounds() {
        let available_rect =
            egui::Rect::from_min_max(egui::pos2(40.0, 30.0), egui::pos2(1040.0, 730.0));

        let rect = workbench_overlay_rect(
            available_rect,
            Some(egui::pos2(-240.0, -180.0)),
            Some(egui::vec2(4000.0, 3000.0)),
        );

        assert!(rect.left() >= available_rect.left() + WORKBENCH_OVERLAY_MARGIN);
        assert!(rect.top() >= available_rect.top() + WORKBENCH_OVERLAY_MARGIN);
        assert!(rect.right() <= available_rect.right() - WORKBENCH_OVERLAY_MARGIN + 0.5);
        assert!(rect.bottom() <= available_rect.bottom() - WORKBENCH_OVERLAY_MARGIN + 0.5);
    }

    #[test]
    fn workbench_overlay_rect_defaults_to_right_aligned_overlay() {
        let available_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1400.0, 900.0));

        let rect = workbench_overlay_rect(available_rect, None, None);
        let default_size = workbench_overlay_default_size(available_rect);

        assert_eq!(rect.size(), default_size);
        assert!(rect.right() <= available_rect.right() - WORKBENCH_OVERLAY_MARGIN + 0.5);
        assert!((rect.center().y - available_rect.center().y).abs() <= 1.0);
    }
}
