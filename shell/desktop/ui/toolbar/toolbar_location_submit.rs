use super::*;
use crate::shell::desktop::ui::toolbar_routing;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_location_submit(
    ui: &egui::Ui,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &Tree<TileKind>,
    focused_toolbar_node: Option<NodeKey>,
    has_webview_tiles: bool,
    is_graph_view: bool,
    location: &mut String,
    location_dirty: &mut bool,
    location_submitted: &mut bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    frame_intents: &mut Vec<GraphIntent>,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
) {
    let should_submit_now = *location_submitted
        || ui.input(|i| i.key_pressed(Key::Enter));
    if !should_submit_now {
        return;
    }

    *location_submitted = false;
    let mut handled_omnibar_search = false;
    let trimmed_location = location.trim().to_string();
    if let Some(query) = trimmed_location.strip_prefix('@') {
        if let Some((provider, provider_query)) = parse_provider_search_query(query) {
            let query = provider_query.trim();
            if query.is_empty() {
                *omnibar_search_session = None;
                *location_dirty = false;
                handled_omnibar_search = true;
            } else {
                *location = query.to_string();
                *omnibar_search_session = None;
                let split_open_requested = ui.input(|i| i.modifiers.shift);
                let submit_result = toolbar_routing::submit_address_bar_intents(
                    graph_app,
                    location,
                    is_graph_view,
                    focused_toolbar_node,
                    split_open_requested,
                    window,
                    searchpage_template_for_provider(provider),
                );
                frame_intents.extend(submit_result.intents);
                if submit_result.mark_clean {
                    *location_dirty = false;
                    *open_selected_mode_after_submit = submit_result.open_mode;
                }
                handled_omnibar_search = true;
            }
        } else {
            let (mode, query) = parse_omnibar_search_query(query);
            if query.is_empty() {
                *omnibar_search_session = None;
                *location_dirty = false;
                handled_omnibar_search = true;
            }

            if !handled_omnibar_search {
                let reuse_existing = omnibar_search_session.as_ref().is_some_and(|session| {
                    session.kind == OmnibarSessionKind::Graph(mode)
                        && session.query == query
                        && !session.matches.is_empty()
                });
                if !reuse_existing {
                    let matches = omnibar_matches_for_query(
                        graph_app,
                        tiles_tree,
                        mode,
                        query,
                        has_webview_tiles,
                    );
                    if matches.is_empty() {
                        *omnibar_search_session = None;
                    } else {
                        *omnibar_search_session = Some(OmnibarSearchSession {
                            kind: OmnibarSessionKind::Graph(mode),
                            query: query.to_string(),
                            matches,
                            active_index: 0,
                            selected_indices: HashSet::new(),
                            anchor_index: None,
                            provider_rx: None,
                            provider_debounce_deadline: None,
                            provider_status: ProviderSuggestionStatus::Idle,
                        });
                    }
                }

                if let Some(session) = omnibar_search_session.as_ref()
                    && !session.matches.is_empty()
                    && let Some(active_match) = session.matches.get(session.active_index).cloned()
                {
                    let shift_override_original = ui.input(|i| i.modifiers.shift);
                    apply_omnibar_match(
                        graph_app,
                        active_match,
                        has_webview_tiles,
                        shift_override_original,
                        frame_intents,
                        open_selected_mode_after_submit,
                    );
                }
                *location_dirty = true;
                handled_omnibar_search = true;
            }
        }
    }

    if !handled_omnibar_search {
        let mut handled_non_at_session = false;
        if let Some(session) = omnibar_search_session.as_ref()
            && matches!(session.kind, OmnibarSessionKind::SearchProvider(_))
            && session.query == trimmed_location.as_str()
            && !session.matches.is_empty()
            && let Some(active_match) = session.matches.get(session.active_index).cloned()
        {
            match active_match {
                OmnibarMatch::SearchQuery { query, provider } => {
                    *location = query;
                    *omnibar_search_session = None;
                    let split_open_requested = ui.input(|i| i.modifiers.shift);
                    let submit_result = toolbar_routing::submit_address_bar_intents(
                        graph_app,
                        location,
                        is_graph_view,
                        focused_toolbar_node,
                        split_open_requested,
                        window,
                        searchpage_template_for_provider(provider),
                    );
                    frame_intents.extend(submit_result.intents);
                    if submit_result.mark_clean {
                        *location_dirty = false;
                        *open_selected_mode_after_submit = submit_result.open_mode;
                    }
                }
                other => {
                    *omnibar_search_session = None;
                    let shift_override_original = ui.input(|i| i.modifiers.shift);
                    apply_omnibar_match(
                        graph_app,
                        other,
                        has_webview_tiles,
                        shift_override_original,
                        frame_intents,
                        open_selected_mode_after_submit,
                    );
                    *location_dirty = true;
                }
            }
            handled_non_at_session = true;
        }

        if !handled_non_at_session {
            *omnibar_search_session = None;
            let split_open_requested = ui.input(|i| i.modifiers.shift);
            let submit_result = toolbar_routing::submit_address_bar_intents(
                graph_app,
                location,
                is_graph_view,
                focused_toolbar_node,
                split_open_requested,
                window,
                &state.app_preferences.searchpage,
            );
            frame_intents.extend(submit_result.intents);
            if submit_result.mark_clean {
                *location_dirty = false;
                *open_selected_mode_after_submit = submit_result.open_mode;
            }
        }
    }
}
