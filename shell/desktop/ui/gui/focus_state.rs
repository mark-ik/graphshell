/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::app::ToolSurfaceReturnTarget;
use crate::shell::desktop::ui::gui_state::{
    EmbeddedContentTarget, FocusCaptureEntry, FocusCaptureSurface, FocusCommand, ReturnAnchor,
    RuntimeFocusAuthorityState, RuntimeFocusInspector, SemanticRegionFocus,
};
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use egui_tiles::Tree;

#[derive(Clone, Copy)]
enum CanvasFocusTarget {
    Node(Option<NodeKey>),
    GraphSurface(Option<GraphViewId>),
}

fn embedded_focus_components(
    focus: Option<&EmbeddedContentTarget>,
) -> (Option<servo::WebViewId>, Option<NodeKey>) {
    match focus {
        Some(EmbeddedContentTarget::WebView {
            renderer_id,
            node_key,
        }) => (Some(*renderer_id), *node_key),
        None => (None, None),
    }
}

pub(crate) fn workspace_runtime_focus_state(
    graph_app: &GraphBrowserApp,
    focus_authority: Option<&RuntimeFocusAuthorityState>,
    local_widget_focus: Option<LocalFocusTarget>,
    show_clear_data_confirm: bool,
) -> RuntimeFocusState {
    build_runtime_focus_state(RuntimeFocusInputs {
        semantic_region_override: focus_authority
            .and_then(|authority| authority.semantic_region.clone()),
        pane_activation: focus_authority.and_then(|authority| authority.pane_activation),
        pane_region_hint: None,
        focused_view: graph_app.workspace.graph_runtime.focused_view,
        focused_node_hint: None,
        graph_surface_focused: false,
        local_widget_focus,
        embedded_content_focus_webview: graph_app.embedded_content_focus_webview(),
        embedded_content_focus_node: graph_app
            .embedded_content_focus_webview()
            .and_then(|webview_id| graph_app.get_node_for_webview(webview_id)),
        show_command_palette: graph_app.workspace.chrome_ui.show_command_palette,
        show_context_palette: graph_app.workspace.chrome_ui.show_context_palette,
        command_palette_contextual_mode: graph_app
            .workspace
            .chrome_ui
            .command_palette_contextual_mode,
        show_help_panel: graph_app.workspace.chrome_ui.show_help_panel,
        show_scene_overlay: graph_app.workspace.chrome_ui.show_scene_overlay,
        show_settings_overlay: graph_app.workspace.chrome_ui.show_settings_overlay,
        show_radial_menu: graph_app.workspace.chrome_ui.show_radial_menu,
        show_clip_inspector: graph_app.workspace.chrome_ui.show_clip_inspector,
        show_clear_data_confirm,
        command_surface_return_target: focus_authority
            .and_then(|authority| authority.command_surface_return_target.clone())
            .or_else(|| graph_app.pending_command_surface_return_target()),
        transient_surface_return_target: focus_authority
            .and_then(|authority| authority.transient_surface_return_target.clone())
            .or_else(|| graph_app.pending_transient_surface_return_target()),
    })
}

pub(crate) fn workbench_runtime_focus_state(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    focus_authority: Option<&RuntimeFocusAuthorityState>,
    local_widget_focus: Option<LocalFocusTarget>,
    show_clear_data_confirm: bool,
) -> RuntimeFocusState {
    let active_target =
        crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(
            tiles_tree,
        );
    let (pane_region_hint, focused_view, focused_node_hint, graph_surface_focused) =
        match active_target {
            Some(ToolSurfaceReturnTarget::Graph(view_id)) => (
                Some(PaneRegionHint::GraphSurface),
                Some(view_id),
                None,
                true,
            ),
            Some(ToolSurfaceReturnTarget::Node(node_key)) => (
                Some(PaneRegionHint::NodePane),
                graph_app.workspace.graph_runtime.focused_view,
                Some(node_key),
                false,
            ),
            Some(ToolSurfaceReturnTarget::Tool(_)) => (
                Some(PaneRegionHint::ToolPane),
                graph_app.workspace.graph_runtime.focused_view,
                None,
                false,
            ),
            None => (
                None,
                graph_app.workspace.graph_runtime.focused_view,
                None,
                false,
            ),
        };

    build_runtime_focus_state(RuntimeFocusInputs {
        semantic_region_override: focus_authority
            .and_then(|authority| authority.semantic_region.clone()),
        pane_activation: focus_authority.and_then(|authority| authority.pane_activation),
        pane_region_hint,
        focused_view,
        focused_node_hint,
        graph_surface_focused,
        local_widget_focus,
        embedded_content_focus_webview: graph_app.embedded_content_focus_webview(),
        embedded_content_focus_node: graph_app
            .embedded_content_focus_webview()
            .and_then(|webview_id| graph_app.get_node_for_webview(webview_id)),
        show_command_palette: graph_app.workspace.chrome_ui.show_command_palette,
        show_context_palette: graph_app.workspace.chrome_ui.show_context_palette,
        command_palette_contextual_mode: graph_app
            .workspace
            .chrome_ui
            .command_palette_contextual_mode,
        show_help_panel: graph_app.workspace.chrome_ui.show_help_panel,
        show_scene_overlay: graph_app.workspace.chrome_ui.show_scene_overlay,
        show_settings_overlay: graph_app.workspace.chrome_ui.show_settings_overlay,
        show_radial_menu: graph_app.workspace.chrome_ui.show_radial_menu,
        show_clip_inspector: graph_app.workspace.chrome_ui.show_clip_inspector,
        show_clear_data_confirm,
        command_surface_return_target: focus_authority
            .and_then(|authority| authority.command_surface_return_target.clone())
            .or_else(|| graph_app.pending_command_surface_return_target()),
        transient_surface_return_target: focus_authority
            .and_then(|authority| authority.transient_surface_return_target.clone())
            .or_else(|| graph_app.pending_transient_surface_return_target()),
    })
}

pub(crate) fn desired_runtime_focus_state(
    graph_app: &GraphBrowserApp,
    focus_authority: &RuntimeFocusAuthorityState,
    local_widget_focus: Option<LocalFocusTarget>,
    show_clear_data_confirm: bool,
) -> RuntimeFocusState {
    let (embedded_content_focus_webview, embedded_content_focus_node) =
        embedded_focus_components(focus_authority.embedded_content_focus.as_ref());
    RuntimeFocusState {
        semantic_region: focus_authority
            .semantic_region
            .clone()
            .unwrap_or(SemanticRegionFocus::Unspecified),
        pane_activation: focus_authority.pane_activation,
        graph_view_focus: match focus_authority.semantic_region.as_ref() {
            Some(SemanticRegionFocus::GraphSurface { view_id }) => {
                view_id.or(graph_app.workspace.graph_runtime.focused_view)
            }
            _ => graph_app.workspace.graph_runtime.focused_view,
        },
        local_widget_focus,
        embedded_content_focus: embedded_content_focus_webview.map(|renderer_id| {
            EmbeddedContentTarget::WebView {
                renderer_id,
                node_key: embedded_content_focus_node,
            }
        }),
        capture_stack: if show_clear_data_confirm {
            let mut stack = focus_authority.capture_stack.clone();
            stack.push(FocusCaptureEntry {
                surface: FocusCaptureSurface::ModalDialog,
                return_anchor: focus_authority.pane_activation.map(ReturnAnchor::Pane),
            });
            stack
        } else {
            focus_authority.capture_stack.clone()
        },
    }
}

pub(crate) fn refresh_realized_runtime_focus_state(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    local_widget_focus: Option<LocalFocusTarget>,
    show_clear_data_confirm: bool,
) {
    let active_target = runtime_active_tool_surface_return_target(tiles_tree);
    let (pane_region_hint, focused_view, focused_node_hint, graph_surface_focused) =
        match active_target {
            Some(ToolSurfaceReturnTarget::Graph(view_id)) => (
                Some(PaneRegionHint::GraphSurface),
                Some(view_id),
                None,
                true,
            ),
            Some(ToolSurfaceReturnTarget::Node(node_key)) => (
                Some(PaneRegionHint::NodePane),
                graph_app.workspace.graph_runtime.focused_view,
                Some(node_key),
                false,
            ),
            Some(ToolSurfaceReturnTarget::Tool(_)) => (
                Some(PaneRegionHint::ToolPane),
                graph_app.workspace.graph_runtime.focused_view,
                None,
                false,
            ),
            None => (
                None,
                graph_app.workspace.graph_runtime.focused_view,
                None,
                false,
            ),
        };
    focus_authority.realized_focus_state = Some(build_runtime_focus_state(RuntimeFocusInputs {
        semantic_region_override: None,
        pane_activation: focus_authority.pane_activation,
        pane_region_hint,
        focused_view,
        focused_node_hint,
        graph_surface_focused,
        local_widget_focus,
        embedded_content_focus_webview: graph_app.embedded_content_focus_webview(),
        embedded_content_focus_node: graph_app
            .embedded_content_focus_webview()
            .and_then(|webview_id| graph_app.get_node_for_webview(webview_id)),
        show_command_palette: graph_app.workspace.chrome_ui.show_command_palette,
        show_context_palette: graph_app.workspace.chrome_ui.show_context_palette,
        command_palette_contextual_mode: graph_app
            .workspace
            .chrome_ui
            .command_palette_contextual_mode,
        show_help_panel: graph_app.workspace.chrome_ui.show_help_panel,
        show_scene_overlay: graph_app.workspace.chrome_ui.show_scene_overlay,
        show_settings_overlay: graph_app.workspace.chrome_ui.show_settings_overlay,
        show_radial_menu: graph_app.workspace.chrome_ui.show_radial_menu,
        show_clip_inspector: graph_app.workspace.chrome_ui.show_clip_inspector,
        show_clear_data_confirm,
        command_surface_return_target: focus_authority.command_surface_return_target.clone(),
        transient_surface_return_target: focus_authority.transient_surface_return_target.clone(),
    }));
}

pub(crate) fn runtime_focus_inspector(
    graph_app: &GraphBrowserApp,
    focus_authority: &RuntimeFocusAuthorityState,
    local_widget_focus: Option<LocalFocusTarget>,
    show_clear_data_confirm: bool,
) -> RuntimeFocusInspector {
    RuntimeFocusInspector {
        desired: desired_runtime_focus_state(
            graph_app,
            focus_authority,
            local_widget_focus.clone(),
            show_clear_data_confirm,
        ),
        realized: focus_authority
            .realized_focus_state
            .clone()
            .unwrap_or_else(|| {
                desired_runtime_focus_state(
                    graph_app,
                    focus_authority,
                    local_widget_focus,
                    show_clear_data_confirm,
                )
            }),
    }
}

pub(super) fn build_runtime_focus_state(inputs: RuntimeFocusInputs) -> RuntimeFocusState {
    let RuntimeFocusInputs {
        semantic_region_override,
        pane_activation,
        pane_region_hint,
        focused_view,
        focused_node_hint,
        graph_surface_focused,
        local_widget_focus,
        embedded_content_focus_webview,
        embedded_content_focus_node,
        show_command_palette,
        show_context_palette,
        command_palette_contextual_mode: _command_palette_contextual_mode,
        show_help_panel,
        show_scene_overlay,
        show_settings_overlay,
        show_radial_menu,
        show_clip_inspector,
        show_clear_data_confirm,
        command_surface_return_target,
        transient_surface_return_target,
    } = inputs;

    let mut capture_stack = Vec::new();
    if show_clear_data_confirm {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::ModalDialog,
            return_anchor: pane_activation.map(ReturnAnchor::Pane),
        });
    }
    if show_command_palette {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::CommandPalette,
            return_anchor: command_surface_return_target
                .clone()
                .map(ReturnAnchor::ToolSurface),
        });
    }
    if show_context_palette {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::ContextPalette,
            return_anchor: command_surface_return_target.map(ReturnAnchor::ToolSurface),
        });
    }
    if show_radial_menu {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::RadialPalette,
            return_anchor: transient_surface_return_target
                .clone()
                .map(ReturnAnchor::ToolSurface),
        });
    }
    if show_scene_overlay {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::SceneOverlay,
            return_anchor: transient_surface_return_target
                .clone()
                .map(ReturnAnchor::ToolSurface),
        });
    }
    if show_settings_overlay {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::SettingsOverlay,
            return_anchor: transient_surface_return_target
                .clone()
                .map(ReturnAnchor::ToolSurface),
        });
    }
    if show_clip_inspector {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::ClipInspector,
            return_anchor: transient_surface_return_target
                .clone()
                .map(ReturnAnchor::ToolSurface),
        });
    }
    if show_help_panel {
        capture_stack.push(FocusCaptureEntry {
            surface: FocusCaptureSurface::HelpPanel,
            return_anchor: transient_surface_return_target.map(ReturnAnchor::ToolSurface),
        });
    }

    let semantic_region = if show_clear_data_confirm {
        SemanticRegionFocus::ModalDialog
    } else if show_context_palette {
        SemanticRegionFocus::ContextPalette
    } else if show_command_palette {
        SemanticRegionFocus::CommandPalette
    } else if show_radial_menu {
        SemanticRegionFocus::RadialPalette
    } else if show_scene_overlay {
        SemanticRegionFocus::SceneOverlay
    } else if show_settings_overlay {
        SemanticRegionFocus::SettingsOverlay
    } else if show_clip_inspector {
        SemanticRegionFocus::ClipInspector
    } else if show_help_panel {
        SemanticRegionFocus::HelpPanel
    } else if let Some(semantic_region) = semantic_region_override {
        semantic_region
    } else if matches!(
        local_widget_focus,
        Some(LocalFocusTarget::ToolbarLocation { .. })
    ) {
        SemanticRegionFocus::Toolbar
    } else if graph_surface_focused {
        SemanticRegionFocus::GraphSurface {
            view_id: focused_view,
        }
    } else {
        match pane_region_hint {
            Some(PaneRegionHint::ToolPane) => SemanticRegionFocus::ToolPane {
                pane_id: pane_activation,
            },
            Some(PaneRegionHint::NodePane) => SemanticRegionFocus::NodePane {
                pane_id: pane_activation,
                node_key: focused_node_hint,
            },
            Some(PaneRegionHint::GraphSurface) => SemanticRegionFocus::GraphSurface {
                view_id: focused_view,
            },
            None if focused_view.is_some() => SemanticRegionFocus::GraphSurface {
                view_id: focused_view,
            },
            None if focused_node_hint.is_some() => SemanticRegionFocus::NodePane {
                pane_id: pane_activation,
                node_key: focused_node_hint,
            },
            None => SemanticRegionFocus::Unspecified,
        }
    };

    RuntimeFocusState {
        semantic_region,
        pane_activation,
        graph_view_focus: focused_view,
        local_widget_focus,
        embedded_content_focus: embedded_content_focus_webview.map(|renderer_id| {
            EmbeddedContentTarget::WebView {
                renderer_id,
                node_key: embedded_content_focus_node,
            }
        }),
        capture_stack,
    }
}

pub(super) fn apply_node_focus_state(
    runtime_state: &mut GraphshellRuntime,
    node_key: Option<NodeKey>,
) {
    apply_canvas_region_focus_state(runtime_state, false, CanvasFocusTarget::Node(node_key));
}

/// `update_focused_view` controls whether the graph runtime's `focused_view` is
/// written alongside the canvas focus transition. The node-focus path leaves it
/// alone; the graph-surface path writes the new view.
fn apply_canvas_region_focus_state(
    runtime_state: &mut GraphshellRuntime,
    update_focused_view: bool,
    target: CanvasFocusTarget,
) {
    let was_focused_node_hint = runtime_state.focused_node_hint;
    let was_graph_surface_focused = runtime_state.graph_surface_focused;
    let was_focused_view = runtime_state.graph_app.workspace.graph_runtime.focused_view;

    match target {
        CanvasFocusTarget::Node(node_key) => {
            runtime_state.focused_node_hint = node_key;
            runtime_state.graph_surface_focused = false;
        }
        CanvasFocusTarget::GraphSurface(next_view) => {
            runtime_state.focused_node_hint = None;
            runtime_state.graph_surface_focused = true;
            if update_focused_view {
                runtime_state.graph_app.workspace.graph_runtime.focused_view = next_view;
            }
        }
    }

    let current_focused_view = runtime_state.graph_app.workspace.graph_runtime.focused_view;
    if runtime_state.focused_node_hint != was_focused_node_hint
        || runtime_state.graph_surface_focused != was_graph_surface_focused
        || current_focused_view != was_focused_view
    {
        runtime_state.focus_authority.semantic_region = match target {
            CanvasFocusTarget::Node(node_key) => Some(SemanticRegionFocus::NodePane {
                pane_id: runtime_state.focus_authority.pane_activation,
                node_key,
            }),
            CanvasFocusTarget::GraphSurface(next_view) => {
                Some(SemanticRegionFocus::GraphSurface { view_id: next_view })
            }
        };
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
}

pub(crate) fn apply_pane_activation_focus_state(
    runtime_state: &mut GraphshellRuntime,
    pane_id: Option<PaneId>,
) {
    runtime_state.focus_authority.pane_activation = pane_id;
    match runtime_state.focus_authority.semantic_region.as_mut() {
        Some(SemanticRegionFocus::NodePane {
            pane_id: semantic_pane_id,
            ..
        })
        | Some(SemanticRegionFocus::ToolPane {
            pane_id: semantic_pane_id,
        }) => {
            *semantic_pane_id = pane_id;
        }
        _ => {}
    }
    if matches!(
        runtime_state.focus_authority.local_widget_focus,
        Some(LocalFocusTarget::ToolbarLocation { .. })
    ) {
        runtime_state.focus_authority.local_widget_focus =
            Some(LocalFocusTarget::ToolbarLocation { pane_id });
        runtime_state.focus_authority.semantic_region = Some(SemanticRegionFocus::Toolbar);
    }
}

pub(crate) fn sync_runtime_focus_authority_from_app(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
) {
    focus_authority.realized_focus_state =
        Some(workspace_runtime_focus_state(graph_app, None, None, false));
}

fn runtime_active_tool_surface_return_target(
    tiles_tree: &Tree<TileKind>,
) -> Option<ToolSurfaceReturnTarget> {
    crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(
        tiles_tree,
    )
}

fn tool_surface_target_is_control_surface(target: &Option<ToolSurfaceReturnTarget>) -> bool {
    matches!(
        target,
        Some(ToolSurfaceReturnTarget::Tool(
            crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings
        )) | Some(ToolSurfaceReturnTarget::Tool(
            crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager
        )) | Some(ToolSurfaceReturnTarget::Tool(
            crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics
        ))
    )
}

pub(crate) fn capture_command_surface_return_target_in_authority(
    focus_authority: &mut RuntimeFocusAuthorityState,
    tiles_tree: &Tree<TileKind>,
) {
    if focus_authority.command_surface_return_target.is_none() {
        focus_authority.command_surface_return_target =
            runtime_active_tool_surface_return_target(tiles_tree);
    }
}

fn semantic_region_for_capture_surface(surface: FocusCaptureSurface) -> SemanticRegionFocus {
    match surface {
        FocusCaptureSurface::ModalDialog => SemanticRegionFocus::ModalDialog,
        FocusCaptureSurface::CommandPalette => SemanticRegionFocus::CommandPalette,
        FocusCaptureSurface::ContextPalette => SemanticRegionFocus::ContextPalette,
        FocusCaptureSurface::RadialPalette => SemanticRegionFocus::RadialPalette,
        FocusCaptureSurface::SceneOverlay => SemanticRegionFocus::SceneOverlay,
        FocusCaptureSurface::SettingsOverlay => SemanticRegionFocus::SettingsOverlay,
        FocusCaptureSurface::ClipInspector => SemanticRegionFocus::ClipInspector,
        FocusCaptureSurface::HelpPanel => SemanticRegionFocus::HelpPanel,
    }
}

pub(crate) fn semantic_region_for_tool_surface_target(
    target: &ToolSurfaceReturnTarget,
) -> SemanticRegionFocus {
    match target {
        ToolSurfaceReturnTarget::Graph(view_id) => SemanticRegionFocus::GraphSurface {
            view_id: Some(*view_id),
        },
        ToolSurfaceReturnTarget::Node(node_key) => SemanticRegionFocus::NodePane {
            pane_id: None,
            node_key: Some(*node_key),
        },
        ToolSurfaceReturnTarget::Tool(_) => SemanticRegionFocus::ToolPane { pane_id: None },
    }
}

pub(crate) fn apply_focus_command(
    focus_authority: &mut RuntimeFocusAuthorityState,
    command: FocusCommand,
) {
    match command {
        FocusCommand::EnterCommandPalette {
            contextual_mode,
            return_target,
        } => {
            let surface = if contextual_mode {
                FocusCaptureSurface::ContextPalette
            } else {
                FocusCaptureSurface::CommandPalette
            };
            focus_authority.capture_stack.push(FocusCaptureEntry {
                surface,
                return_anchor: return_target
                    .as_ref()
                    .map(|t| ReturnAnchor::ToolSurface(t.clone())),
            });
            focus_authority.semantic_region = Some(semantic_region_for_capture_surface(surface));
            if let Some(return_target) = return_target {
                focus_authority.command_surface_return_target = Some(return_target);
            }
        }
        FocusCommand::ExitCommandPalette => {
            focus_authority.capture_stack.retain(|entry| {
                !matches!(
                    entry.surface,
                    FocusCaptureSurface::CommandPalette | FocusCaptureSurface::ContextPalette
                )
            });
            if matches!(
                &focus_authority.semantic_region,
                Some(SemanticRegionFocus::CommandPalette | SemanticRegionFocus::ContextPalette)
            ) {
                if let Some(top) = focus_authority.capture_stack.last() {
                    focus_authority.semantic_region =
                        Some(semantic_region_for_capture_surface(top.surface));
                } else {
                    focus_authority.semantic_region = focus_authority
                        .command_surface_return_target
                        .as_ref()
                        .map(semantic_region_for_tool_surface_target);
                }
            }
        }
        FocusCommand::EnterTransientSurface {
            surface,
            return_target,
        } => {
            focus_authority.capture_stack.push(FocusCaptureEntry {
                surface,
                return_anchor: return_target
                    .as_ref()
                    .map(|target| ReturnAnchor::ToolSurface(target.clone())),
            });
            focus_authority.semantic_region = Some(semantic_region_for_capture_surface(surface));
            if let Some(return_target) = return_target {
                focus_authority.transient_surface_return_target = Some(return_target);
            }
        }
        FocusCommand::ExitTransientSurface {
            surface,
            restore_target,
        } => {
            focus_authority
                .capture_stack
                .retain(|entry| entry.surface != surface);
            if matches!(
                &focus_authority.semantic_region,
                Some(
                    SemanticRegionFocus::RadialPalette
                        | SemanticRegionFocus::SceneOverlay
                        | SemanticRegionFocus::SettingsOverlay
                        | SemanticRegionFocus::HelpPanel
                )
            ) {
                if let Some(top) = focus_authority.capture_stack.last() {
                    focus_authority.semantic_region =
                        Some(semantic_region_for_capture_surface(top.surface));
                } else {
                    focus_authority.semantic_region = restore_target
                        .as_ref()
                        .map(semantic_region_for_tool_surface_target);
                }
            }
            focus_authority.transient_surface_return_target = restore_target;
        }
        FocusCommand::SetEmbeddedContentFocus { target } => {
            focus_authority.embedded_content_focus = target;
        }
        FocusCommand::EnterToolPane { return_target } => {
            focus_authority.semantic_region = Some(SemanticRegionFocus::ToolPane { pane_id: None });
            if let Some(return_target) = return_target {
                focus_authority.tool_surface_return_target = Some(return_target);
            }
        }
        FocusCommand::ExitToolPane { restore_target } => {
            focus_authority.semantic_region = restore_target
                .as_ref()
                .map(semantic_region_for_tool_surface_target);
            focus_authority.tool_surface_return_target = restore_target;
        }
        FocusCommand::SetSemanticRegion { region } => {
            focus_authority.semantic_region = Some(region);
        }
        FocusCommand::Capture {
            surface,
            return_anchor,
        } => {
            focus_authority.capture_stack.push(FocusCaptureEntry {
                surface,
                return_anchor,
            });
            focus_authority.semantic_region = Some(semantic_region_for_capture_surface(surface));
        }
        FocusCommand::RestoreCapturedFocus { surface } => {
            focus_authority
                .capture_stack
                .retain(|entry| entry.surface != surface);
            if let Some(top) = focus_authority.capture_stack.last() {
                focus_authority.semantic_region =
                    Some(semantic_region_for_capture_surface(top.surface));
            } else {
                focus_authority.semantic_region = None;
            }
        }
    }
}

pub(crate) fn capture_tool_surface_return_target_in_authority(
    focus_authority: &mut RuntimeFocusAuthorityState,
    tiles_tree: &Tree<TileKind>,
) {
    let active_target = runtime_active_tool_surface_return_target(tiles_tree);
    if !tool_surface_target_is_control_surface(&active_target) {
        focus_authority.tool_surface_return_target = active_target;
    }
}

pub(crate) fn seed_command_surface_return_target_from_authority(
    focus_authority: &RuntimeFocusAuthorityState,
    graph_app: &mut GraphBrowserApp,
) {
    if graph_app.pending_command_surface_return_target().is_none() {
        graph_app.set_pending_command_surface_return_target(
            focus_authority.command_surface_return_target.clone(),
        );
    }
}

pub(crate) fn seed_tool_surface_return_target_from_authority(
    focus_authority: &RuntimeFocusAuthorityState,
    graph_app: &mut GraphBrowserApp,
) {
    if graph_app.pending_tool_surface_return_target().is_none() {
        graph_app.set_pending_tool_surface_return_target(
            focus_authority.tool_surface_return_target.clone(),
        );
    }
}

pub(crate) fn seed_transient_surface_return_target_from_authority(
    focus_authority: &RuntimeFocusAuthorityState,
    graph_app: &mut GraphBrowserApp,
) {
    if graph_app
        .pending_transient_surface_return_target()
        .is_none()
    {
        graph_app.set_pending_transient_surface_return_target(
            focus_authority.transient_surface_return_target.clone(),
        );
    }
}

pub(super) fn sync_runtime_focus_authority_state(runtime: &mut GraphshellRuntime) {
    let local_widget_focus = runtime.focus_authority.local_widget_focus.clone();
    let realized =
        workspace_runtime_focus_state(&runtime.graph_app, None, local_widget_focus, false);
    runtime.focus_authority.realized_focus_state = Some(realized);
}

pub(crate) fn sync_runtime_semantic_region_from_workbench(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    local_widget_focus: Option<LocalFocusTarget>,
    show_clear_data_confirm: bool,
) {
    if let Some(target) = runtime_active_tool_surface_return_target(tiles_tree) {
        if matches!(
            target,
            ToolSurfaceReturnTarget::Node(_) | ToolSurfaceReturnTarget::Tool(_)
        ) {
            focus_authority.last_non_graph_pane_activation = focus_authority.pane_activation;
        }
        focus_authority.semantic_region = Some(semantic_region_for_tool_surface_target(&target));
    }
    refresh_realized_runtime_focus_state(
        focus_authority,
        graph_app,
        tiles_tree,
        local_widget_focus,
        show_clear_data_confirm,
    );
}

pub(crate) fn realize_embedded_content_focus_from_authority(
    focus_authority: &RuntimeFocusAuthorityState,
    graph_app: &mut GraphBrowserApp,
) {
    let webview_id = match focus_authority.embedded_content_focus.as_ref() {
        Some(EmbeddedContentTarget::WebView { renderer_id, .. }) => Some(*renderer_id),
        None => None,
    };
    graph_app.set_embedded_content_focus_webview(webview_id);
}

pub(crate) fn apply_graph_search_local_focus_state(
    graph_search_open: &mut bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    open: bool,
) {
    *graph_search_open = open;
    if open {
        *local_widget_focus = Some(LocalFocusTarget::GraphSearch);
    } else if matches!(*local_widget_focus, Some(LocalFocusTarget::GraphSearch)) {
        *local_widget_focus = None;
    }
}

pub(crate) fn apply_toolbar_location_local_focus_state(
    runtime_state: &mut GraphshellRuntime,
    focused: bool,
) {
    if focused {
        runtime_state.focus_authority.local_widget_focus =
            Some(LocalFocusTarget::ToolbarLocation {
                pane_id: runtime_state.focus_authority.pane_activation,
            });
        runtime_state.focus_authority.semantic_region = Some(SemanticRegionFocus::Toolbar);
    } else if matches!(
        runtime_state.focus_authority.local_widget_focus,
        Some(LocalFocusTarget::ToolbarLocation { .. })
    ) {
        runtime_state.focus_authority.local_widget_focus = None;
        if matches!(
            &runtime_state.focus_authority.semantic_region,
            Some(SemanticRegionFocus::Toolbar)
        ) {
            runtime_state.focus_authority.semantic_region = None;
        }
    }
}

pub(super) fn apply_graph_surface_focus_state(
    runtime_state: &mut GraphshellRuntime,
    active_graph_view: Option<GraphViewId>,
) {
    apply_canvas_region_focus_state(
        runtime_state,
        true,
        CanvasFocusTarget::GraphSurface(active_graph_view),
    );
}

pub(super) fn ui_overlay_active_from_flags(
    show_command_palette: bool,
    show_context_palette: bool,
    show_help_panel: bool,
    show_scene_overlay: bool,
    show_settings_overlay: bool,
    show_radial_menu: bool,
    show_clip_inspector: bool,
    show_clear_data_confirm: bool,
) -> bool {
    build_runtime_focus_state(RuntimeFocusInputs {
        semantic_region_override: None,
        pane_activation: None,
        pane_region_hint: None,
        focused_view: None,
        focused_node_hint: None,
        graph_surface_focused: false,
        local_widget_focus: None,
        embedded_content_focus_webview: None,
        embedded_content_focus_node: None,
        show_command_palette,
        show_context_palette,
        command_palette_contextual_mode: show_context_palette,
        show_help_panel,
        show_scene_overlay,
        show_settings_overlay,
        show_radial_menu,
        show_clip_inspector,
        show_clear_data_confirm,
        command_surface_return_target: None,
        transient_surface_return_target: None,
    })
    .overlay_active()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;
    use crate::app::ToolSurfaceReturnTarget;
    use crate::graph::NodeKey;
    use crate::shell::desktop::ui::gui_state::{
        EmbeddedContentTarget, FocusCaptureEntry, FocusCaptureSurface, ReturnAnchor,
        SemanticRegionFocus,
    };
    use crate::shell::desktop::workbench::pane_model::PaneId;
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState};
    use base::id::{PIPELINE_NAMESPACE, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(base::id::PainterId::next())
    }

    #[test]
    fn runtime_focus_state_models_all_six_tracks_for_context_palette() {
        let pane_id = PaneId::new();
        let graph_view = GraphViewId::new();
        let node_key = NodeKey::new(17);
        let webview_id = test_webview_id();
        let return_target = ToolSurfaceReturnTarget::Graph(graph_view);

        let state = build_runtime_focus_state(RuntimeFocusInputs {
            semantic_region_override: None,
            pane_activation: Some(pane_id),
            pane_region_hint: Some(PaneRegionHint::NodePane),
            focused_view: Some(graph_view),
            focused_node_hint: Some(node_key),
            graph_surface_focused: false,
            local_widget_focus: Some(LocalFocusTarget::ToolbarLocation {
                pane_id: Some(pane_id),
            }),
            embedded_content_focus_webview: Some(webview_id),
            embedded_content_focus_node: Some(node_key),
            show_command_palette: false,
            show_context_palette: true,
            command_palette_contextual_mode: true,
            show_help_panel: false,
            show_scene_overlay: false,
            show_settings_overlay: false,
            show_radial_menu: false,
            show_clip_inspector: false,
            show_clear_data_confirm: false,
            command_surface_return_target: Some(return_target.clone()),
            transient_surface_return_target: None,
        });

        assert_eq!(state.semantic_region, SemanticRegionFocus::ContextPalette);
        assert_eq!(state.pane_activation, Some(pane_id));
        assert_eq!(state.graph_view_focus, Some(graph_view));
        assert_eq!(
            state.local_widget_focus,
            Some(LocalFocusTarget::ToolbarLocation {
                pane_id: Some(pane_id)
            })
        );
        assert_eq!(
            state.embedded_content_focus,
            Some(EmbeddedContentTarget::WebView {
                renderer_id: webview_id,
                node_key: Some(node_key)
            })
        );
        assert_eq!(state.capture_stack.len(), 1);
        assert_eq!(
            state.capture_stack[0],
            FocusCaptureEntry {
                surface: FocusCaptureSurface::ContextPalette,
                return_anchor: Some(ReturnAnchor::ToolSurface(return_target)),
            }
        );
    }

    #[test]
    fn runtime_focus_state_uses_modal_capture_stack_for_overlay_activity() {
        let state = build_runtime_focus_state(RuntimeFocusInputs {
            semantic_region_override: None,
            pane_activation: None,
            pane_region_hint: None,
            focused_view: None,
            focused_node_hint: None,
            graph_surface_focused: false,
            local_widget_focus: None,
            embedded_content_focus_webview: None,
            embedded_content_focus_node: None,
            show_command_palette: false,
            show_context_palette: false,
            command_palette_contextual_mode: false,
            show_help_panel: false,
            show_scene_overlay: false,
            show_settings_overlay: false,
            show_radial_menu: false,
            show_clip_inspector: false,
            show_clear_data_confirm: true,
            command_surface_return_target: None,
            transient_surface_return_target: None,
        });

        assert!(state.overlay_active());
        assert_eq!(state.semantic_region, SemanticRegionFocus::ModalDialog);
        assert_eq!(state.capture_stack.len(), 1);
        assert_eq!(
            state.capture_stack[0].surface,
            FocusCaptureSurface::ModalDialog
        );
    }

    #[test]
    fn workspace_runtime_focus_state_tracks_command_surface_capture() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app.workspace.chrome_ui.show_context_palette = true;
        app.workspace.chrome_ui.command_palette_contextual_mode = true;
        app.set_pending_command_surface_return_target(Some(ToolSurfaceReturnTarget::Graph(
            view_id,
        )));

        let state = workspace_runtime_focus_state(&app, None, None, false);

        assert_eq!(state.semantic_region, SemanticRegionFocus::ContextPalette);
        assert!(state.overlay_active());
        assert_eq!(state.graph_view_focus, Some(view_id));
        assert_eq!(state.capture_stack.len(), 1);
        assert_eq!(
            state.capture_stack[0],
            FocusCaptureEntry {
                surface: FocusCaptureSurface::ContextPalette,
                return_anchor: Some(ReturnAnchor::ToolSurface(ToolSurfaceReturnTarget::Graph(
                    view_id
                ))),
            }
        );
    }

    #[test]
    fn workspace_runtime_focus_state_tracks_explicit_toolbar_local_focus() {
        let app = GraphBrowserApp::new_for_testing();
        let pane_id = PaneId::new();

        let state = workspace_runtime_focus_state(
            &app,
            None,
            Some(LocalFocusTarget::ToolbarLocation {
                pane_id: Some(pane_id),
            }),
            false,
        );

        assert_eq!(state.semantic_region, SemanticRegionFocus::Toolbar);
        assert_eq!(
            state.local_widget_focus,
            Some(LocalFocusTarget::ToolbarLocation {
                pane_id: Some(pane_id),
            })
        );
    }

    #[test]
    fn workbench_runtime_focus_state_tracks_active_node_region() {
        let mut app = GraphBrowserApp::new_for_testing();
        let graph_view = GraphViewId::new();
        app.workspace.graph_runtime.focused_view = Some(graph_view);
        let node_key = NodeKey::new(29);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let mut tree = Tree::new("workbench_focus_state_node", root, tiles);
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
        );

        let state = workbench_runtime_focus_state(&app, &tree, None, None, false);

        assert_eq!(
            state.semantic_region,
            SemanticRegionFocus::NodePane {
                pane_id: None,
                node_key: Some(node_key),
            }
        );
        assert_eq!(state.graph_view_focus, Some(graph_view));
    }

    #[test]
    fn graph_search_local_focus_helper_updates_open_state() {
        let mut graph_search_open = false;
        let mut local_widget_focus = None;

        apply_graph_search_local_focus_state(&mut graph_search_open, &mut local_widget_focus, true);
        assert!(graph_search_open);
        assert_eq!(local_widget_focus, Some(LocalFocusTarget::GraphSearch));

        apply_graph_search_local_focus_state(
            &mut graph_search_open,
            &mut local_widget_focus,
            false,
        );
        assert!(!graph_search_open);
        assert_eq!(local_widget_focus, None);
    }

    #[test]
    fn pane_activation_focus_helper_updates_active_toolbar_pane() {
        let mut runtime_state = GraphshellRuntime::for_testing();
        let pane_id = PaneId::new();

        apply_pane_activation_focus_state(&mut runtime_state, Some(pane_id));

        assert_eq!(runtime_state.focus_authority.pane_activation, Some(pane_id));
    }

    #[test]
    fn toolbar_location_focus_helper_updates_local_widget_focus() {
        let pane_id = PaneId::new();
        let mut runtime_state = GraphshellRuntime::for_testing();
        runtime_state.focus_authority.pane_activation = Some(pane_id);

        apply_toolbar_location_local_focus_state(&mut runtime_state, true);
        assert_eq!(
            runtime_state.focus_authority.local_widget_focus,
            Some(LocalFocusTarget::ToolbarLocation {
                pane_id: Some(pane_id),
            })
        );

        apply_toolbar_location_local_focus_state(&mut runtime_state, false);
        assert_eq!(runtime_state.focus_authority.local_widget_focus, None);
    }

    #[test]
    fn runtime_focus_authority_sync_tracks_realized_focus_without_overwriting_desired_authority() {
        let mut runtime_state = GraphshellRuntime::for_testing();
        let graph_view = GraphViewId::new();
        let node_key = NodeKey::new(77);

        runtime_state.focus_authority.command_surface_return_target =
            Some(ToolSurfaceReturnTarget::Graph(graph_view));
        runtime_state.focus_authority.semantic_region = Some(SemanticRegionFocus::CommandPalette);
        runtime_state
            .graph_app
            .set_pending_tool_surface_return_target(Some(ToolSurfaceReturnTarget::Graph(
                graph_view,
            )));
        runtime_state
            .graph_app
            .set_pending_command_surface_return_target(Some(ToolSurfaceReturnTarget::Node(
                node_key,
            )));
        runtime_state
            .graph_app
            .set_pending_transient_surface_return_target(Some(ToolSurfaceReturnTarget::Graph(
                graph_view,
            )));

        sync_runtime_focus_authority_state(&mut runtime_state);

        assert_eq!(
            runtime_state.focus_authority.command_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );
        assert_eq!(
            runtime_state.focus_authority.semantic_region,
            Some(SemanticRegionFocus::CommandPalette)
        );
        assert_eq!(
            runtime_state
                .focus_authority
                .realized_focus_state
                .as_ref()
                .map(|state| state.semantic_region.clone()),
            Some(SemanticRegionFocus::Unspecified)
        );
    }

    #[test]
    fn focus_command_updates_command_palette_authority() {
        let graph_view = GraphViewId::new();
        let mut focus_authority =
            crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default();

        apply_focus_command(
            &mut focus_authority,
            FocusCommand::EnterCommandPalette {
                contextual_mode: false,
                return_target: Some(ToolSurfaceReturnTarget::Graph(graph_view)),
            },
        );
        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::CommandPalette)
        );
        assert_eq!(
            focus_authority.command_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );

        apply_focus_command(&mut focus_authority, FocusCommand::ExitCommandPalette);
        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::GraphSurface {
                view_id: Some(graph_view),
            })
        );
        assert_eq!(
            focus_authority.command_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );
    }

    #[test]
    fn focus_command_updates_tool_pane_authority() {
        let graph_view = GraphViewId::new();
        let mut focus_authority =
            crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default();

        apply_focus_command(
            &mut focus_authority,
            FocusCommand::EnterToolPane {
                return_target: Some(ToolSurfaceReturnTarget::Graph(graph_view)),
            },
        );
        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::ToolPane { pane_id: None })
        );
        assert_eq!(
            focus_authority.tool_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );

        apply_focus_command(
            &mut focus_authority,
            FocusCommand::ExitToolPane {
                restore_target: Some(ToolSurfaceReturnTarget::Graph(graph_view)),
            },
        );
        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::GraphSurface {
                view_id: Some(graph_view),
            })
        );
        assert_eq!(
            focus_authority.tool_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );
    }

    #[test]
    fn focus_command_updates_transient_surface_authority() {
        let graph_view = GraphViewId::new();
        let mut focus_authority =
            crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default();

        apply_focus_command(
            &mut focus_authority,
            FocusCommand::EnterTransientSurface {
                surface: FocusCaptureSurface::HelpPanel,
                return_target: Some(ToolSurfaceReturnTarget::Graph(graph_view)),
            },
        );
        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::HelpPanel)
        );
        assert_eq!(
            focus_authority.transient_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );

        apply_focus_command(
            &mut focus_authority,
            FocusCommand::ExitTransientSurface {
                surface: FocusCaptureSurface::HelpPanel,
                restore_target: Some(ToolSurfaceReturnTarget::Graph(graph_view)),
            },
        );
        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::GraphSurface {
                view_id: Some(graph_view),
            })
        );
        assert_eq!(
            focus_authority.transient_surface_return_target,
            Some(ToolSurfaceReturnTarget::Graph(graph_view))
        );
    }

    #[test]
    fn focus_command_updates_embedded_content_authority() {
        let node_key = NodeKey::new(818);
        let webview_id = test_webview_id();
        let mut focus_authority =
            crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default();

        apply_focus_command(
            &mut focus_authority,
            FocusCommand::SetEmbeddedContentFocus {
                target: Some(EmbeddedContentTarget::WebView {
                    renderer_id: webview_id,
                    node_key: Some(node_key),
                }),
            },
        );

        assert_eq!(
            focus_authority.embedded_content_focus,
            Some(EmbeddedContentTarget::WebView {
                renderer_id: webview_id,
                node_key: Some(node_key),
            })
        );
    }

    #[test]
    fn runtime_focus_inspector_distinguishes_desired_and_realized_focus() {
        let graph_view = GraphViewId::new();
        let desired_node = NodeKey::new(901);
        let realized_node = NodeKey::new(902);
        let app = GraphBrowserApp::new_for_testing();
        let inspector = runtime_focus_inspector(
            &app,
            &crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState {
                semantic_region: Some(SemanticRegionFocus::NodePane {
                    pane_id: None,
                    node_key: Some(desired_node),
                }),
                realized_focus_state: Some(RuntimeFocusState {
                    semantic_region: SemanticRegionFocus::GraphSurface {
                        view_id: Some(graph_view),
                    },
                    pane_activation: None,
                    graph_view_focus: Some(graph_view),
                    local_widget_focus: None,
                    embedded_content_focus: Some(EmbeddedContentTarget::WebView {
                        renderer_id: test_webview_id(),
                        node_key: Some(realized_node),
                    }),
                    capture_stack: Vec::new(),
                }),
                ..Default::default()
            },
            None,
            false,
        );

        assert_eq!(
            inspector.desired.semantic_region,
            SemanticRegionFocus::NodePane {
                pane_id: None,
                node_key: Some(desired_node),
            }
        );
        assert_eq!(
            inspector.realized.semantic_region,
            SemanticRegionFocus::GraphSurface {
                view_id: Some(graph_view),
            }
        );
    }

    #[test]
    fn runtime_semantic_region_sync_tracks_active_workbench_region() {
        let mut app = GraphBrowserApp::new_for_testing();
        let graph_view = GraphViewId::new();
        app.workspace.graph_runtime.focused_view = Some(graph_view);
        let node_key = NodeKey::new(91);
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let mut tree = Tree::new("runtime_semantic_region_sync", root, tiles);
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
        );
        let mut focus_authority =
            crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default();

        sync_runtime_semantic_region_from_workbench(&mut focus_authority, &app, &tree, None, false);

        assert_eq!(
            focus_authority.semantic_region,
            Some(SemanticRegionFocus::NodePane {
                pane_id: None,
                node_key: Some(node_key),
            })
        );
    }
}
