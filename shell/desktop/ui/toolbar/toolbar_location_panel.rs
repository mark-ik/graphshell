use super::*;
use super::toolbar_location_dropdown;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_location_search_panel(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
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
    focus_location_field_for_search: bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    frame_intents: &mut Vec<GraphIntent>,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
) {
    let location_id = egui::Id::new("location_input");
    let location_field = ui.add_sized(
        ui.available_size(),
        egui::TextEdit::singleline(location)
            .id(location_id)
            .hint_text("Search or enter address"),
    );

    if location_field.changed() {
        *location_dirty = true;
    }
    if focus_location_field_for_search
        || ui.input(|i| {
            if cfg!(target_os = "macos") {
                i.clone().consume_key(Modifiers::COMMAND, Key::L)
            } else {
                i.clone().consume_key(Modifiers::COMMAND, Key::L)
                    || i.clone().consume_key(Modifiers::ALT, Key::D)
            }
        })
    {
        location_field.request_focus();
    }
    if location_field.gained_focus()
        && let Some(mut state) = TextEditState::load(ui.ctx(), location_id)
    {
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(0),
            CCursor::new(location.len()),
        )));
        state.store(ui.ctx(), location_id);
    }

    if location_field.has_focus() {
        let trimmed_location = location.trim();
        if let Some(query_raw) = trimmed_location.strip_prefix('@') {
            if let Some((provider, provider_query)) = parse_provider_search_query(query_raw) {
                let query = provider_query.trim();
                if query.is_empty() {
                    *omnibar_search_session = None;
                } else {
                    let needs_refresh = !omnibar_search_session.as_ref().is_some_and(|session| {
                        session.kind == OmnibarSessionKind::SearchProvider(provider)
                            && session.query == trimmed_location
                    });
                    if needs_refresh {
                        *omnibar_search_session = Some(OmnibarSearchSession {
                            kind: OmnibarSessionKind::SearchProvider(provider),
                            query: trimmed_location.to_string(),
                            matches: vec![OmnibarMatch::SearchQuery {
                                query: query.to_string(),
                                provider,
                            }],
                            active_index: 0,
                            selected_indices: HashSet::new(),
                            anchor_index: None,
                            provider_rx: None,
                            provider_debounce_deadline: Some(
                                Instant::now() + Duration::from_millis(OMNIBAR_PROVIDER_DEBOUNCE_MS),
                            ),
                            provider_status: ProviderSuggestionStatus::Loading,
                        });
                    }
                }
            } else {
                let (mode, query) = parse_omnibar_search_query(query_raw);
                if query.is_empty() {
                    *omnibar_search_session = None;
                } else {
                    let needs_refresh = !omnibar_search_session.as_ref().is_some_and(|session| {
                        session.kind == OmnibarSessionKind::Graph(mode) && session.query == query
                    });
                    if needs_refresh {
                        let matches = omnibar_matches_for_query(
                            graph_app,
                            tiles_tree,
                            mode,
                            query,
                            has_webview_tiles,
                        );
                        *omnibar_search_session = if matches.is_empty() {
                            None
                        } else {
                            Some(OmnibarSearchSession {
                                kind: OmnibarSessionKind::Graph(mode),
                                query: query.to_string(),
                                matches,
                                active_index: 0,
                                selected_indices: HashSet::new(),
                                anchor_index: None,
                                provider_rx: None,
                                provider_debounce_deadline: None,
                                provider_status: ProviderSuggestionStatus::Idle,
                            })
                        };
                    }
                }
            }
        } else if trimmed_location.len() >= OMNIBAR_PROVIDER_MIN_QUERY_LEN {
            let provider =
                default_search_provider_from_searchpage(&state.app_preferences.searchpage)
                    .unwrap_or(SearchProviderKind::DuckDuckGo);
            let (initial_matches, should_fetch_provider) =
                non_at_matches_for_settings(graph_app, tiles_tree, trimmed_location, has_webview_tiles);
            let initial_status = if should_fetch_provider {
                ProviderSuggestionStatus::Loading
            } else {
                ProviderSuggestionStatus::Ready
            };
            let initial_deadline = if should_fetch_provider {
                Some(Instant::now() + Duration::from_millis(OMNIBAR_PROVIDER_DEBOUNCE_MS))
            } else {
                None
            };
            let needs_refresh = !omnibar_search_session.as_ref().is_some_and(|session| {
                session.kind == OmnibarSessionKind::SearchProvider(provider)
                    && session.query == trimmed_location
            });
            if needs_refresh {
                *omnibar_search_session = Some(OmnibarSearchSession {
                    kind: OmnibarSessionKind::SearchProvider(provider),
                    query: trimmed_location.to_string(),
                    matches: initial_matches,
                    active_index: 0,
                    selected_indices: HashSet::new(),
                    anchor_index: None,
                    provider_rx: None,
                    provider_debounce_deadline: initial_deadline,
                    provider_status: initial_status,
                });
            }
        } else if trimmed_location.is_empty() {
            let local_workspace_tab_matches = omnibar_matches_for_query(
                graph_app,
                tiles_tree,
                OmnibarSearchMode::TabsLocal,
                "",
                has_webview_tiles,
            );
            let provider = default_search_provider_from_searchpage(&state.app_preferences.searchpage)
                .unwrap_or(SearchProviderKind::DuckDuckGo);
            *omnibar_search_session = Some(OmnibarSearchSession {
                kind: OmnibarSessionKind::SearchProvider(provider),
                query: String::new(),
                matches: local_workspace_tab_matches,
                active_index: 0,
                selected_indices: HashSet::new(),
                anchor_index: None,
                provider_rx: None,
                provider_debounce_deadline: None,
                provider_status: ProviderSuggestionStatus::Idle,
            });
        } else {
            *omnibar_search_session = None;
        }
    }

    if let Some(session) = omnibar_search_session.as_mut()
        && matches!(session.kind, OmnibarSessionKind::SearchProvider(_))
        && location_field.has_focus()
        && session.query == location.trim()
    {
        if let Some(deadline) = session.provider_debounce_deadline
            && session.provider_rx.is_none()
            && Instant::now() >= deadline
            && let OmnibarSessionKind::SearchProvider(provider) = session.kind
        {
            session.provider_debounce_deadline = None;
            session.provider_rx = Some(spawn_provider_suggestion_request(provider, &session.query));
        }

        let mut fetched_outcome = None;
        if let Some(rx) = &session.provider_rx {
            match rx.try_recv() {
                Ok(outcome) => fetched_outcome = Some(outcome),
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(75));
                },
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    fetched_outcome = Some(ProviderSuggestionFetchOutcome {
                        matches: Vec::new(),
                        status: ProviderSuggestionStatus::Failed(ProviderSuggestionError::Network),
                    });
                },
            }
        }
        if session.provider_debounce_deadline.is_some() {
            ctx.request_repaint_after(Duration::from_millis(75));
        }
        if let Some(outcome) = fetched_outcome {
            session.provider_rx = None;
            session.provider_status = outcome.status;
            if !session.query.starts_with('@') {
                let fallback_scope = if graph_app.workspace.omnibar_preferred_scope
                    == OmnibarPreferredScope::ProviderDefault
                {
                    OmnibarPreferredScope::Auto
                } else {
                    graph_app.workspace.omnibar_preferred_scope
                };
                let primary_matches = non_at_primary_matches_for_scope(
                    graph_app,
                    tiles_tree,
                    &session.query,
                    has_webview_tiles,
                    fallback_scope,
                );
                match graph_app.workspace.omnibar_non_at_order {
                    OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal => {
                        session.matches.extend(outcome.matches);
                    },
                    OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => {
                        if outcome.matches.is_empty() {
                            session.matches = primary_matches;
                        } else {
                            session.matches = outcome.matches;
                            session.matches.extend(primary_matches);
                        }
                    },
                }
            } else {
                session.matches.extend(outcome.matches);
            }
            session.matches = dedupe_matches_in_order(session.matches.clone());
            if session.matches.is_empty() && !session.query.starts_with('@') {
                session.matches = non_at_global_fallback_matches(
                    graph_app,
                    tiles_tree,
                    &session.query,
                    has_webview_tiles,
                );
            }
            if !session.matches.is_empty()
                && !matches!(
                    session.provider_status,
                    ProviderSuggestionStatus::Failed(_)
                )
            {
                session.provider_status = ProviderSuggestionStatus::Ready;
            }
            session.active_index = session.active_index.min(session.matches.len().saturating_sub(1));
        }
    }

    // Delegate dropdown rendering to focused helper module
    toolbar_location_dropdown::render_omnibar_dropdown(
        ctx,
        ui,
        &location_field,
        location,
        location_dirty,
        omnibar_search_session,
        graph_app,
        tiles_tree,
        is_graph_view,
        focused_toolbar_node,
        window,
        has_webview_tiles,
        frame_intents,
        open_selected_mode_after_submit,
    );

    let enter_while_focused = location_field.has_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if enter_while_focused {
        *location_submitted = true;
    }
    if enter_while_focused
        || *location_submitted
        || (location_field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
    {
        super::toolbar_location_submit::handle_location_submit(
            ui,
            state,
            graph_app,
            window,
            tiles_tree,
            focused_toolbar_node,
            has_webview_tiles,
            is_graph_view,
            location,
            location_dirty,
            location_submitted,
            omnibar_search_session,
            frame_intents,
            open_selected_mode_after_submit,
        );
    }
}
