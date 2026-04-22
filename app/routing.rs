use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRouteTarget {
    History,
    Settings(SettingsToolPage),
}

// `ToolSurfaceReturnTarget` moved to `graphshell_core::routing` in
// M4 slice 10 (2026-04-22). Re-exported here so existing `app::
// ToolSurfaceReturnTarget` import paths resolve unchanged.
pub use graphshell_core::routing::ToolSurfaceReturnTarget;

impl GraphBrowserApp {
    pub fn resolve_settings_route(url: &str) -> Option<SettingsRouteTarget> {
        match VersoAddress::parse(url)? {
            VersoAddress::Settings(GraphshellSettingsPath::History) => {
                Some(SettingsRouteTarget::History)
            }
            VersoAddress::Settings(GraphshellSettingsPath::General) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::General))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Persistence) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Persistence))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Physics) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Physics))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Sync) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Sync))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Appearance) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Appearance))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Keybindings) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Keybindings))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Advanced) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Advanced))
            }
            VersoAddress::Frame(_)
            | VersoAddress::TileGroup(_)
            | VersoAddress::View(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Clip(_)
            | VersoAddress::Settings(GraphshellSettingsPath::Other(_))
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn apply_settings_route_target(
        &mut self,
        route: SettingsRouteTarget,
    ) -> crate::shell::desktop::workbench::pane_model::ToolPaneState {
        match route {
            SettingsRouteTarget::History => {
                crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager
            }
            SettingsRouteTarget::Settings(page) => {
                self.workspace.chrome_ui.settings_tool_page = page;
                crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings
            }
        }
    }

    pub fn resolve_frame_route(url: &str) -> Option<String> {
        match VersoAddress::parse(url)? {
            VersoAddress::Frame(frame_name) => Some(frame_name),
            VersoAddress::Settings(_)
            | VersoAddress::TileGroup(_)
            | VersoAddress::View(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Clip(_)
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_tool_route(
        url: &str,
    ) -> Option<crate::shell::desktop::workbench::pane_model::ToolPaneState> {
        match VersoAddress::parse(url)? {
            VersoAddress::Tool { name, .. } => match name.as_str() {
                "diagnostics" => Some(crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics),
                "history" => Some(crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager),
                "accessibility" => Some(
                    crate::shell::desktop::workbench::pane_model::ToolPaneState::AccessibilityInspector,
                ),
                "settings" => Some(crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings),
                _ => None,
            },
            VersoAddress::Settings(_)
            | VersoAddress::Frame(_)
            | VersoAddress::TileGroup(_)
            | VersoAddress::View(_)
            | VersoAddress::Clip(_)
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_view_route(url: &str) -> Option<ViewRouteTarget> {
        match VersoAddress::parse(url)? {
            VersoAddress::View(VersoViewTarget::Legacy(view_id)) => {
                let parsed = Uuid::parse_str(&view_id).ok()?;
                Some(ViewRouteTarget::GraphPane(GraphViewId::from_uuid(parsed)))
            }
            VersoAddress::View(VersoViewTarget::Graph(graph_id)) => {
                Some(ViewRouteTarget::Graph(graph_id))
            }
            VersoAddress::View(VersoViewTarget::Note(note_id)) => {
                let parsed = Uuid::parse_str(&note_id).ok()?;
                Some(ViewRouteTarget::Note(NoteId::from_uuid(parsed)))
            }
            VersoAddress::View(VersoViewTarget::Node(node_id)) => {
                let parsed = Uuid::parse_str(&node_id).ok()?;
                Some(ViewRouteTarget::Node(parsed))
            }
            VersoAddress::Settings(_)
            | VersoAddress::Frame(_)
            | VersoAddress::TileGroup(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Clip(_)
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_graph_route(url: &str) -> Option<String> {
        GraphAddress::parse(url).map(|address| address.graph_id)
    }

    pub fn resolve_node_route(url: &str) -> Option<Uuid> {
        let address = NodeAddress::parse(url)?;
        Uuid::parse_str(&address.node_id).ok()
    }

    pub fn resolve_clip_route(url: &str) -> Option<String> {
        match VersoAddress::parse(url)? {
            VersoAddress::Clip(clip_id) => Some(clip_id),
            VersoAddress::Settings(_)
            | VersoAddress::Frame(_)
            | VersoAddress::TileGroup(_)
            | VersoAddress::View(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_note_route(url: &str) -> Option<NoteId> {
        let address = NoteAddress::parse(url)?;
        let parsed = Uuid::parse_str(&address.note_id).ok()?;
        Some(NoteId::from_uuid(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_settings_route_history() {
        let result = GraphBrowserApp::resolve_settings_route("graphshell://settings/history");
        assert_eq!(result, Some(SettingsRouteTarget::History));
    }

    #[test]
    fn resolve_settings_route_general() {
        let result = GraphBrowserApp::resolve_settings_route("graphshell://settings/general");
        assert_eq!(
            result,
            Some(SettingsRouteTarget::Settings(SettingsToolPage::General))
        );
    }

    #[test]
    fn resolve_settings_route_non_settings_url() {
        let result = GraphBrowserApp::resolve_settings_route("https://example.com");
        assert_eq!(result, None);
    }

    #[test]
    fn apply_settings_route_target_history_returns_history_manager() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.chrome_ui.settings_tool_page = SettingsToolPage::Advanced;

        let target = app.apply_settings_route_target(SettingsRouteTarget::History);

        assert_eq!(
            target,
            crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager
        );
        assert_eq!(
            app.workspace.chrome_ui.settings_tool_page,
            SettingsToolPage::Advanced
        );
    }

    #[test]
    fn apply_settings_route_target_settings_updates_page_and_returns_settings_pane() {
        let mut app = GraphBrowserApp::new_for_testing();

        let target = app
            .apply_settings_route_target(SettingsRouteTarget::Settings(SettingsToolPage::Physics));

        assert_eq!(
            target,
            crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings
        );
        assert_eq!(
            app.workspace.chrome_ui.settings_tool_page,
            SettingsToolPage::Physics
        );
    }

    #[test]
    fn resolve_frame_route_valid() {
        let result = GraphBrowserApp::resolve_frame_route("graphshell://frame/my-frame");
        assert_eq!(result, Some("my-frame".to_string()));
    }

    #[test]
    fn resolve_frame_route_non_frame_url() {
        let result = GraphBrowserApp::resolve_frame_route("graphshell://settings/general");
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_graph_route_valid() {
        let result = GraphBrowserApp::resolve_graph_route("graph://my-graph");
        assert_eq!(result, Some("my-graph".to_string()));
    }

    #[test]
    fn resolve_node_route_valid() {
        let id = uuid::Uuid::new_v4();
        let url = format!("node://{id}");
        let result = GraphBrowserApp::resolve_node_route(&url);
        assert_eq!(result, Some(id));
    }

    #[test]
    fn resolve_node_route_invalid_uuid() {
        let result = GraphBrowserApp::resolve_node_route("node://not-a-uuid");
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_clip_route_valid() {
        let result = GraphBrowserApp::resolve_clip_route("graphshell://clip/my-clip");
        assert_eq!(result, Some("my-clip".to_string()));
    }

    #[test]
    fn resolve_note_route_valid() {
        let id = uuid::Uuid::new_v4();
        let url = format!("notes://{id}");
        let result = GraphBrowserApp::resolve_note_route(&url);
        assert_eq!(result, Some(NoteId::from_uuid(id)));
    }
}
