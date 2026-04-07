use super::*;

pub(super) struct FocusRealizer<'a> {
    graph_app: &'a mut GraphBrowserApp,
    tiles_tree: &'a mut Tree<TileKind>,
}

impl<'a> FocusRealizer<'a> {
    pub(super) fn new(
        graph_app: &'a mut GraphBrowserApp,
        tiles_tree: &'a mut Tree<TileKind>,
    ) -> Self {
        Self {
            graph_app,
            tiles_tree,
        }
    }

    pub(super) fn realize_workbench_intent(
        &mut self,
        focus_authority: &mut RuntimeFocusAuthorityState,
        intent: &WorkbenchIntent,
    ) -> Option<WorkbenchIntent> {
        match intent {
            WorkbenchIntent::OpenCommandPalette => {
                self.open_command_palette_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::CloseCommandPalette => {
                self.close_command_palette_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::ToggleCommandPalette
                if self.graph_app.workspace.chrome_ui.show_command_palette
                    || self.graph_app.workspace.chrome_ui.show_context_palette =>
            {
                self.close_command_palette_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::ToggleCommandPalette => {
                self.open_command_palette_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::ToggleHelpPanel
                if self.graph_app.workspace.chrome_ui.show_help_panel =>
            {
                self.close_transient_surface_from_authority(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                );
                None
            }
            WorkbenchIntent::ToggleHelpPanel => {
                self.open_help_panel_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::CloseHelpPanel => {
                self.close_transient_surface_from_authority(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                );
                None
            }
            WorkbenchIntent::ToggleRadialMenu
                if self.graph_app.workspace.chrome_ui.show_radial_menu =>
            {
                self.close_transient_surface_from_authority(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                );
                None
            }
            WorkbenchIntent::ToggleRadialMenu => {
                self.open_radial_menu_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::CloseRadialMenu => {
                self.close_transient_surface_from_authority(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                );
                None
            }
            WorkbenchIntent::CycleFocusRegion => {
                self.realize_semantic_region_from_focus_authority(focus_authority);
                None
            }
            WorkbenchIntent::SetWorkbenchOverlayVisible { visible: true } => {
                self.open_workbench_overlay_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::SetWorkbenchOverlayVisible { visible: false } => {
                self.close_workbench_overlay_from_authority(focus_authority);
                None
            }
            WorkbenchIntent::OpenSettingsUrl { url } => {
                if let Some(crate::app::SettingsRouteTarget::Settings(page)) =
                    GraphBrowserApp::resolve_settings_route(url)
                    && crate::shell::desktop::runtime::registries::workbench_surface::settings_url_targets_overlay(
                        self.graph_app,
                        self.tiles_tree,
                        url,
                    )
                {
                    crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                        focus_authority,
                        self.graph_app,
                    );
                    self.graph_app.open_settings_overlay(page);
                    None
                } else {
                    dispatch_workbench_authority_intent(
                        self.graph_app,
                        self.tiles_tree,
                        intent.clone(),
                    )
                }
            }
            WorkbenchIntent::OpenToolPane { kind } => {
                self.open_tool_pane_from_authority(focus_authority, kind);
                None
            }
            WorkbenchIntent::CloseToolPane {
                kind,
                restore_previous_focus,
            } => {
                self.close_tool_pane_from_authority(focus_authority, kind, *restore_previous_focus);
                None
            }
            _ => {
                dispatch_workbench_authority_intent(self.graph_app, self.tiles_tree, intent.clone())
            }
        }
    }

    fn open_command_palette_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_command_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        if matches!(
            focus_authority.semantic_region,
            Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette)
        ) {
            self.graph_app.open_context_palette();
        } else {
            self.graph_app.open_command_palette();
        }
        true
    }

    fn close_command_palette_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_command_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        self.graph_app.close_command_palette();
        let target = self.graph_app.take_pending_command_surface_return_target();
        let _ = self.restore_focus_target_or_ensure_active_tile(target, true);
        true
    }

    fn open_help_panel_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        self.graph_app.open_help_panel();
        true
    }

    fn open_radial_menu_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        self.graph_app.open_radial_menu();
        true
    }

    fn open_workbench_overlay_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        self.graph_app.open_workbench_overlay();
        self.realize_semantic_region_from_focus_authority(focus_authority);
        true
    }

    fn close_workbench_overlay_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        self.graph_app.close_workbench_overlay();
        let target = self.graph_app.take_pending_tool_surface_return_target();
        let _ = self.restore_focus_target_or_ensure_active_tile(target, true);
        true
    }

    fn close_transient_surface_from_authority(
        &mut self,
        focus_authority: &mut RuntimeFocusAuthorityState,
        surface: crate::shell::desktop::ui::gui_state::FocusCaptureSurface,
    ) -> bool {
        crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        match surface {
            crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel => {
                self.graph_app.close_help_panel();
            }
            crate::shell::desktop::ui::gui_state::FocusCaptureSurface::SceneOverlay => {
                self.graph_app.close_scene_overlay();
            }
            crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette => {
                self.graph_app.close_radial_menu();
            }
            _ => return false,
        }
        self.restore_pending_transient_surface_focus(focus_authority);
        true
    }

    fn open_tool_pane_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
        kind: &ToolPaneState,
    ) -> bool {
        if matches!(
            kind,
            ToolPaneState::Settings | ToolPaneState::HistoryManager
        ) {
            crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
                focus_authority,
                self.graph_app,
            );
        }
        dispatch_workbench_authority_intent(
            self.graph_app,
            self.tiles_tree,
            WorkbenchIntent::OpenToolPane { kind: kind.clone() },
        )
        .is_none()
    }

    fn close_tool_pane_from_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
        kind: &ToolPaneState,
        restore_previous_focus: bool,
    ) -> bool {
        if restore_previous_focus {
            crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
                focus_authority,
                self.graph_app,
            );
        }
        dispatch_workbench_authority_intent(
            self.graph_app,
            self.tiles_tree,
            WorkbenchIntent::CloseToolPane {
                kind: kind.clone(),
                restore_previous_focus,
            },
        )
        .is_none()
    }

    fn realize_semantic_region_from_focus_authority(
        &mut self,
        focus_authority: &RuntimeFocusAuthorityState,
    ) -> bool {
        match focus_authority.semantic_region.as_ref() {
            Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::GraphSurface {
                view_id,
            }) => {
                if let Some(view_id) = view_id {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(
                            tile,
                            egui_tiles::Tile::Pane(TileKind::Graph(existing))
                                if existing.graph_view_id == *view_id
                        )
                    })
                } else {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(tile, egui_tiles::Tile::Pane(TileKind::Graph(_)))
                    })
                }
            }
            Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane {
                pane_id,
                node_key,
            }) => {
                if let Some(pane_id) = pane_id {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(
                            tile,
                            egui_tiles::Tile::Pane(TileKind::Node(existing))
                                if existing.pane_id == *pane_id
                        )
                    })
                } else if let Some(node_key) = node_key {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(
                            tile,
                            egui_tiles::Tile::Pane(TileKind::Node(existing))
                                if existing.node == *node_key
                        )
                    })
                } else {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(tile, egui_tiles::Tile::Pane(TileKind::Node(_)))
                    })
                }
            }
            Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane {
                pane_id,
            }) => {
                if let Some(pane_id) = pane_id {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(
                            tile,
                            egui_tiles::Tile::Pane(TileKind::Tool(existing))
                                if existing.pane_id == *pane_id
                        )
                    })
                } else {
                    self.tiles_tree.make_active(|_, tile| {
                        matches!(tile, egui_tiles::Tile::Pane(TileKind::Tool(_)))
                    })
                }
            }
            _ => false,
        }
    }

    fn restore_focus_target_or_ensure_active_tile(
        &mut self,
        target: Option<crate::app::ToolSurfaceReturnTarget>,
        preserve_active_fallback: bool,
    ) -> bool {
        crate::shell::desktop::runtime::registries::workbench_surface::restore_focus_target_or_ensure_active_tile(
            self.graph_app,
            self.tiles_tree,
            target,
            preserve_active_fallback,
        )
    }

    pub(super) fn restore_pending_transient_surface_focus(
        &mut self,
        focus_authority: &mut RuntimeFocusAuthorityState,
    ) {
        if self.graph_app.workspace.chrome_ui.show_command_palette
            || self.graph_app.workspace.chrome_ui.show_context_palette
            || self.graph_app.workspace.chrome_ui.show_help_panel
            || self.graph_app.workspace.chrome_ui.show_settings_overlay
            || self.graph_app.workspace.chrome_ui.show_radial_menu
        {
            return;
        }

        if !self
            .graph_app
            .take_pending_restore_transient_surface_focus()
        {
            return;
        }

        crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
            focus_authority,
            self.graph_app,
        );
        // Clear the authority's transient target after seeding to the app queue so
        // the target is consumed exactly once and subsequent frames do not re-restore.
        focus_authority.transient_surface_return_target = None;

        let focus_before = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
            self.graph_app,
            self.tiles_tree,
            Some(focus_authority),
            None,
            false,
        );
        let target = self
            .graph_app
            .take_pending_transient_surface_return_target();
        let desired_semantic_region = target
            .as_ref()
            .map(crate::shell::desktop::ui::gui::semantic_region_for_tool_surface_target);
        let restored = self.restore_focus_target_or_ensure_active_tile(target.clone(), true);
        if target.is_some() && restored {
            let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
                self.graph_app,
                self.tiles_tree,
                Some(focus_authority),
                None,
                false,
            );
            let restored_target = crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(
                self.tiles_tree,
            );
            if focus_before == focus_after || restored_target != target {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UX_FOCUS_RETURN_FALLBACK,
                    latency_us: 0,
                });
            }
        } else if target.is_some()
            && crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(
                self.tiles_tree,
            )
            .is_some()
        {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UX_FOCUS_RETURN_FALLBACK,
                latency_us: 0,
            });
        } else if target.is_none() && restored {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UX_FOCUS_RETURN_FALLBACK,
                latency_us: 0,
            });
        }
        let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
            self.graph_app,
            self.tiles_tree,
            Some(focus_authority),
            None,
            false,
        );
        refresh_runtime_focus_authority_after_workbench_intent(
            focus_authority,
            self.graph_app,
            self.tiles_tree,
            false,
        );
        if desired_semantic_region
            .as_ref()
            .is_some_and(|desired| *desired != focus_after.semantic_region)
        {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_FOCUS_REALIZATION_MISMATCH,
                byte_len: 1,
            });
        }
        let focus_transitioned = restored && focus_before != focus_after;

        if !focus_transitioned
            && crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(
                self.tiles_tree,
            )
            .is_none()
        {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
                byte_len: 1,
            });
        }
    }
}
