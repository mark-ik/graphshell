/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use egui_tiles::Tree;
use servo::WebViewId;

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::ui::gui_state::ToolbarEditable;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;

const CLEAR_DATA_CONFIRM_WINDOW_SECS: f64 = 3.0;
const CLEAR_DATA_CONFIRM_WARNING_TEXT: &str =
    "Choose 'Clear graph and saved data' again within 3 seconds to confirm";
const CLEAR_DATA_CONFIRM_SUCCESS_TEXT: &str = "Cleared graph and saved data";

#[derive(Clone, Copy, Debug, PartialEq)]
enum ClearDataConfirmAction {
    Arm { next_deadline: f64 },
    Execute,
}

pub(crate) struct DialogPanelsArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) viewer_surfaces:
        &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    pub(crate) viewer_surface_host: &'a mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) frame_intents: &'a mut Vec<GraphIntent>,
    pub(crate) editable: &'a mut ToolbarEditable,
    pub(crate) show_clear_data_confirm: &'a mut bool,
    pub(crate) clear_data_confirm_deadline_secs: &'a mut Option<f64>,
    pub(crate) toasts: &'a mut egui_notify::Toasts,
}

pub(crate) fn render_dialog_panels(args: DialogPanelsArgs<'_>) {
    if *args.show_clear_data_confirm {
        let now = args.ctx.input(|i| i.time);
        match classify_clear_data_confirm_action(now, *args.clear_data_confirm_deadline_secs) {
            ClearDataConfirmAction::Execute => {
                args.frame_intents
                    .extend(webview_controller::close_all_webviews(
                        args.graph_app,
                        args.window,
                    ));
                tile_runtime::reset_runtime_webview_state(
                    args.tiles_tree,
                    args.viewer_surfaces,
                    args.viewer_surface_host,
                    args.tile_favicon_textures,
                    args.favicon_textures,
                );
                args.graph_app.clear_graph_and_persistence();
                args.editable.location_dirty = false;
                args.editable.location_submitted = false;
                *args.clear_data_confirm_deadline_secs = None;
                args.toasts.success(CLEAR_DATA_CONFIRM_SUCCESS_TEXT);
            }
            ClearDataConfirmAction::Arm { next_deadline } => {
                *args.clear_data_confirm_deadline_secs = Some(next_deadline);
                args.toasts.warning(CLEAR_DATA_CONFIRM_WARNING_TEXT);
            }
        }
        *args.show_clear_data_confirm = false;
    }
}

fn clear_data_confirm_is_armed(now: f64, armed_deadline: Option<f64>) -> bool {
    armed_deadline.is_some_and(|deadline| deadline >= now)
}

fn next_clear_data_confirm_deadline(now: f64) -> f64 {
    now + CLEAR_DATA_CONFIRM_WINDOW_SECS
}

fn classify_clear_data_confirm_action(
    now: f64,
    armed_deadline: Option<f64>,
) -> ClearDataConfirmAction {
    if clear_data_confirm_is_armed(now, armed_deadline) {
        ClearDataConfirmAction::Execute
    } else {
        ClearDataConfirmAction::Arm {
            next_deadline: next_clear_data_confirm_deadline(now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CLEAR_DATA_CONFIRM_SUCCESS_TEXT, CLEAR_DATA_CONFIRM_WARNING_TEXT,
        CLEAR_DATA_CONFIRM_WINDOW_SECS, ClearDataConfirmAction, classify_clear_data_confirm_action,
        clear_data_confirm_is_armed, next_clear_data_confirm_deadline,
    };

    #[test]
    fn clear_data_confirm_is_not_armed_without_deadline() {
        assert!(!clear_data_confirm_is_armed(10.0, None));
    }

    #[test]
    fn clear_data_confirm_is_armed_until_deadline_inclusive() {
        let now = 10.0;
        assert!(clear_data_confirm_is_armed(now, Some(now + 0.5)));
        assert!(clear_data_confirm_is_armed(now, Some(now)));
    }

    #[test]
    fn clear_data_confirm_expires_after_deadline_passes() {
        assert!(!clear_data_confirm_is_armed(10.001, Some(10.0)));
    }

    #[test]
    fn next_clear_data_confirm_deadline_uses_expected_window() {
        let now = 7.25;
        assert_eq!(
            next_clear_data_confirm_deadline(now),
            now + CLEAR_DATA_CONFIRM_WINDOW_SECS
        );
    }

    #[test]
    fn clear_data_confirm_warning_text_includes_instruction_and_timing() {
        assert!(CLEAR_DATA_CONFIRM_WARNING_TEXT.contains("Clear graph and saved data"));
        assert!(CLEAR_DATA_CONFIRM_WARNING_TEXT.contains("within 3 seconds"));
    }

    #[test]
    fn clear_data_confirm_success_text_describes_completed_action() {
        assert!(CLEAR_DATA_CONFIRM_SUCCESS_TEXT.contains("Cleared graph"));
    }

    #[test]
    fn clear_data_confirm_action_arms_when_no_deadline_is_present() {
        let now = 13.0;
        assert_eq!(
            classify_clear_data_confirm_action(now, None),
            ClearDataConfirmAction::Arm {
                next_deadline: now + CLEAR_DATA_CONFIRM_WINDOW_SECS,
            }
        );
    }

    #[test]
    fn clear_data_confirm_action_executes_when_deadline_is_active() {
        let now = 13.0;
        assert_eq!(
            classify_clear_data_confirm_action(now, Some(now + 0.1)),
            ClearDataConfirmAction::Execute
        );
    }

    #[test]
    fn clear_data_confirm_action_rearms_when_deadline_expired() {
        let now = 13.0;
        assert_eq!(
            classify_clear_data_confirm_action(now, Some(now - 0.001)),
            ClearDataConfirmAction::Arm {
                next_deadline: now + CLEAR_DATA_CONFIRM_WINDOW_SECS,
            }
        );
    }
}
