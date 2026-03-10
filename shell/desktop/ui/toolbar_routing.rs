/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::app::{BrowserCommand, BrowserCommandTarget, GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::runtime::registries::input::binding_id;
use crate::shell::desktop::ui::nav_targeting;

pub(crate) enum ToolbarNavAction {
    Back,
    Forward,
    Reload,
}

pub(crate) enum ToolbarOpenMode {
    Tab,
    SplitHorizontal,
}

pub(crate) struct ToolbarSubmitResult {
    pub(crate) intents: Vec<GraphIntent>,
    pub(crate) mark_clean: bool,
    pub(crate) open_mode: Option<ToolbarOpenMode>,
}

pub(crate) fn run_nav_action(
    graph_app: &mut GraphBrowserApp,
    _window: &EmbedderWindow,
    focused_toolbar_node: Option<NodeKey>,
    action: ToolbarNavAction,
) -> bool {
    let binding_id = match action {
        ToolbarNavAction::Back => binding_id::toolbar::NAV_BACK,
        ToolbarNavAction::Forward => binding_id::toolbar::NAV_FORWARD,
        ToolbarNavAction::Reload => binding_id::toolbar::NAV_RELOAD,
    };
    if !registries::phase2_resolve_input_binding(binding_id) {
        return false;
    }

    let command = match action {
        ToolbarNavAction::Back => BrowserCommand::Back,
        ToolbarNavAction::Forward => BrowserCommand::Forward,
        ToolbarNavAction::Reload => BrowserCommand::Reload,
    };
    let target = BrowserCommandTarget::ChromeProjection {
        fallback_node: nav_targeting::chrome_projection_node(graph_app, _window)
            .or(focused_toolbar_node),
    };
    graph_app.request_browser_command(target, command);
    true
}

pub(crate) fn submit_address_bar_intents(
    graph_app: &GraphBrowserApp,
    location: &str,
    is_graph_view: bool,
    focused_toolbar_node: Option<NodeKey>,
    split_open_requested: bool,
    window: &EmbedderWindow,
    searchpage: &str,
) -> ToolbarSubmitResult {
    if !registries::phase2_resolve_toolbar_submit_binding() {
        return ToolbarSubmitResult {
            intents: Vec::new(),
            mark_clean: false,
            open_mode: None,
        };
    }

    let submit_result = webview_controller::handle_address_bar_submit_intents(
        graph_app,
        location,
        is_graph_view,
        focused_toolbar_node,
        window,
        searchpage,
    );
    ToolbarSubmitResult {
        intents: submit_result.intents,
        mark_clean: submit_result.outcome.mark_clean,
        open_mode: requested_open_mode(
            submit_result.outcome.open_selected_tile,
            split_open_requested,
        ),
    }
}

fn requested_open_mode(
    open_selected_tile: bool,
    split_open_requested: bool,
) -> Option<ToolbarOpenMode> {
    if !open_selected_tile {
        return None;
    }
    Some(if split_open_requested {
        ToolbarOpenMode::SplitHorizontal
    } else {
        ToolbarOpenMode::Tab
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requested_open_mode_none_when_not_requested() {
        assert!(requested_open_mode(false, false).is_none());
        assert!(requested_open_mode(false, true).is_none());
    }

    #[test]
    fn test_requested_open_mode_tab_when_split_not_requested() {
        assert!(matches!(
            requested_open_mode(true, false),
            Some(ToolbarOpenMode::Tab)
        ));
    }

    #[test]
    fn test_requested_open_mode_split_when_requested() {
        assert!(matches!(
            requested_open_mode(true, true),
            Some(ToolbarOpenMode::SplitHorizontal)
        ));
    }
}
