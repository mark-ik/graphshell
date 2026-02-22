use super::*;

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
                let fallback_scope = if graph_app.omnibar_preferred_scope
                    == OmnibarPreferredScope::ProviderDefault
                {
                    OmnibarPreferredScope::Auto
                } else {
                    graph_app.omnibar_preferred_scope
                };
                let primary_matches = non_at_primary_matches_for_scope(
                    graph_app,
                    tiles_tree,
                    &session.query,
                    has_webview_tiles,
                    fallback_scope,
                );
                match graph_app.omnibar_non_at_order {
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

    let mut overlay_meta: Option<(usize, usize, OmnibarMatch)> = None;
    if let Some(session) = omnibar_search_session.as_mut()
        && location_field.has_focus()
        && session.query == location.trim()
        && !session.matches.is_empty()
    {
        if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
            session.active_index = (session.active_index + 1) % session.matches.len();
        }
        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
            session.active_index = if session.active_index == 0 {
                session.matches.len() - 1
            } else {
                session.active_index - 1
            };
        }
        if let Some(active_match) = session.matches.get(session.active_index).cloned() {
            overlay_meta = Some((session.active_index, session.matches.len(), active_match));
        }
    }
    if let Some((active_index, total, active_match)) = overlay_meta {
        let counter = format!("{}/{}", active_index + 1, total);
        let pos = location_field.rect.right_top() + Vec2::new(-8.0, 4.0);
        ui.painter().text(
            pos,
            egui::Align2::RIGHT_TOP,
            counter,
            egui::FontId::proportional(11.0),
            egui::Color32::GRAY,
        );
        let tag = omnibar_match_signifier(graph_app, tiles_tree, &active_match);
        let tag_pos = pos + Vec2::new(0.0, 12.0);
        ui.painter().text(
            tag_pos,
            egui::Align2::RIGHT_TOP,
            tag,
            egui::FontId::proportional(10.0),
            egui::Color32::from_gray(150),
        );
    }

    let mut clicked_omnibar_match: Option<OmnibarMatch> = None;
    let mut clicked_omnibar_index_with_modifiers: Option<(usize, Modifiers)> = None;
    let mut bulk_open_selected = false;
    let mut bulk_add_selected_to_workspace = false;
    let mut clicked_scope_prefix: Option<&'static str> = None;
    if let Some(session) = omnibar_search_session.as_mut()
        && location_field.has_focus()
        && session.query == location.trim()
    {
        session.selected_indices.retain(|idx| *idx < session.matches.len());
        if session.anchor_index.is_some_and(|idx| idx >= session.matches.len()) {
            session.anchor_index = None;
        }
        let dropdown_pos = location_field.rect.left_bottom() + Vec2::new(0.0, 2.0);
        egui::Area::new(egui::Id::new("omnibar_dropdown"))
            .order(egui::Order::Foreground)
            .fixed_pos(dropdown_pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(location_field.rect.width());
                    let row_count = session.matches.len().min(OMNIBAR_DROPDOWN_MAX_ROWS);
                    for idx in 0..row_count {
                        let active = idx == session.active_index;
                        let selected = session.selected_indices.contains(&idx);
                        let m = session.matches[idx].clone();
                        let label = omnibar_match_label(graph_app, &m);
                        let signifier = omnibar_match_signifier(graph_app, tiles_tree, &m);
                        let row = ui.horizontal(|ui| {
                            let selected_label = ui.selectable_label(active || selected, label);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.small(signifier);
                                },
                            );
                            selected_label
                        });
                        let response = row.inner;
                        if response.hovered() {
                            session.active_index = idx;
                        }
                        if response.clicked() {
                            let modifiers = ui.input(|i| i.modifiers);
                            if modifiers.ctrl || modifiers.shift {
                                clicked_omnibar_index_with_modifiers = Some((idx, modifiers));
                            } else {
                                clicked_omnibar_match = Some(m);
                            }
                        }
                    }
                    if !session.selected_indices.is_empty() {
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.small(format!("{} selected", session.selected_indices.len()));
                            if ui.small_button("Open Selected").clicked() {
                                bulk_open_selected = true;
                            }
                            if ui.small_button("Add Selected To Workspace...").clicked() {
                                bulk_add_selected_to_workspace = true;
                            }
                        });
                    }
                    if row_count > 0 {
                        ui.separator();
                    }
                    if let Some(status) = provider_status_label(session.provider_status) {
                        ui.small(status);
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (label, prefix) in [
                            ("@n", "@n "),
                            ("@N", "@N "),
                            ("@t", "@t "),
                            ("@T", "@T "),
                            ("@g", "@g "),
                            ("@b", "@b "),
                            ("@d", "@d "),
                        ] {
                            if ui.small_button(label).clicked() {
                                clicked_scope_prefix = Some(prefix);
                            }
                        }
                    });
                });
            });
    }

    if let Some((idx, modifiers)) = clicked_omnibar_index_with_modifiers
        && let Some(session) = omnibar_search_session.as_mut()
    {
        if modifiers.shift {
            let anchor = session.anchor_index.unwrap_or(idx);
            if !modifiers.ctrl {
                session.selected_indices.clear();
            }
            if let Some(range) = inclusive_index_range(anchor, idx, session.matches.len()) {
                for selected_idx in range {
                    session.selected_indices.insert(selected_idx);
                }
            }
            session.anchor_index = Some(anchor);
        } else if modifiers.ctrl {
            if !session.selected_indices.insert(idx) {
                session.selected_indices.remove(&idx);
            }
            session.anchor_index = Some(idx);
        }
        session.active_index = idx;
    }

    if bulk_open_selected && let Some(session) = omnibar_search_session.as_ref() {
        let mut ordered: Vec<usize> = session.selected_indices.iter().copied().collect();
        ordered.sort_unstable();
        for idx in ordered {
            if let Some(item) = session.matches.get(idx).cloned() {
                apply_omnibar_match(
                    graph_app,
                    item,
                    has_webview_tiles,
                    false,
                    frame_intents,
                    open_selected_mode_after_submit,
                );
            }
        }
        *location_dirty = true;
    }

    if bulk_add_selected_to_workspace && let Some(session) = omnibar_search_session.as_ref() {
        let mut node_keys = Vec::new();
        let mut ordered: Vec<usize> = session.selected_indices.iter().copied().collect();
        ordered.sort_unstable();
        for idx in ordered {
            if let Some(OmnibarMatch::Node(key)) = session.matches.get(idx) {
                node_keys.push(*key);
            }
        }
        node_keys.sort_by_key(|key| key.index());
        node_keys.dedup();
        if !node_keys.is_empty() {
            graph_app.request_add_exact_selection_to_workspace_picker(node_keys);
        }
    }

    if let Some(prefix) = clicked_scope_prefix {
        *location = prefix.to_string();
        *location_dirty = true;
        *omnibar_search_session = None;
    }

    if let Some(active_match) = clicked_omnibar_match {
        match active_match {
            OmnibarMatch::SearchQuery { query, provider } => {
                *location = query;
                *omnibar_search_session = None;
                let split_open_requested = ui.input(|i| i.modifiers.shift);
                let provider_searchpage = searchpage_template_for_provider(provider);
                let submit_result = toolbar_routing::submit_address_bar_intents(
                    graph_app,
                    location,
                    is_graph_view,
                    focused_toolbar_node,
                    split_open_requested,
                    window,
                    provider_searchpage,
                );
                frame_intents.extend(submit_result.intents);
                if submit_result.mark_clean {
                    *location_dirty = false;
                    *open_selected_mode_after_submit = submit_result.open_mode;
                }
            },
            other => {
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
            },
        }
    }

    let enter_while_focused = location_field.has_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if enter_while_focused {
        *location_submitted = true;
    }
    let should_submit_now = enter_while_focused
        || *location_submitted
        || (location_field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
    if should_submit_now {
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
                    },
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
                    },
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
}
