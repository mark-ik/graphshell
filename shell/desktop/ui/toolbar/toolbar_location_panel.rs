use super::toolbar_location_dropdown;
use super::*;
use crate::shell::desktop::ui::gui_state::LocalFocusTarget;
use crate::shell::desktop::ui::gui_state::toolbar_location_input_id;
use crate::shell::desktop::ui::navigator_context::NavigatorContextProjection;

const LOCATION_INPUT_HINT_TEXT: &str = "Search or enter address";
const LOCATION_INPUT_HEIGHT: f32 = 28.0;

fn provider_cache_key(provider: SearchProviderKind, query: &str) -> String {
    let provider_key = match provider {
        SearchProviderKind::DuckDuckGo => "duckduckgo",
        SearchProviderKind::Bing => "bing",
        SearchProviderKind::Google => "google",
    };
    format!("provider:{provider_key}:{}", query.trim())
}

fn provider_query_for_session(session: &OmnibarSearchSession) -> String {
    if let Some(raw) = session.query.strip_prefix('@')
        && let Some((_provider, query)) = parse_provider_search_query(raw)
    {
        let trimmed = query.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    session.query.trim().to_string()
}

fn outcome_from_cached_suggestions(
    provider: SearchProviderKind,
    suggestions: &[String],
) -> ProviderSuggestionFetchOutcome {
    ProviderSuggestionFetchOutcome {
        matches: suggestions
            .iter()
            .cloned()
            .map(|query| OmnibarMatch::SearchQuery { query, provider })
            .collect(),
        status: ProviderSuggestionStatus::Ready,
    }
}

fn provider_mailbox_for_query(
    request_query: impl Into<String>,
    should_fetch_provider: bool,
) -> ProviderSuggestionMailbox {
    if should_fetch_provider {
        ProviderSuggestionMailbox::debounced(
            request_query.into(),
            Instant::now() + Duration::from_millis(OMNIBAR_PROVIDER_DEBOUNCE_MS),
        )
    } else {
        ProviderSuggestionMailbox::ready()
    }
}

fn search_provider_session(
    provider: SearchProviderKind,
    query: impl Into<String>,
    matches: Vec<OmnibarMatch>,
    request_query: impl Into<String>,
    should_fetch_provider: bool,
) -> OmnibarSearchSession {
    OmnibarSearchSession::new_search_provider(
        provider,
        query,
        matches,
        provider_mailbox_for_query(request_query, should_fetch_provider),
    )
}

fn provider_query_matches_mailbox(session: &OmnibarSearchSession) -> bool {
    session
        .provider_mailbox
        .request_query
        .as_deref()
        .is_none_or(|request_query| request_query == provider_query_for_session(session))
}

fn should_dispatch_location_submit(
    enter_while_focused: bool,
    location_submitted: bool,
    _enter_after_focus_loss: bool,
) -> bool {
    enter_while_focused || location_submitted
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_location_search_panel(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    control_panel: &mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    window: &EmbedderWindow,
    tiles_tree: &Tree<TileKind>,
    command_bar_focus_target: CommandBarFocusTarget,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    has_node_panes: bool,
    is_graph_view: bool,
    navigator_ctx: &NavigatorContextProjection,
    location: &mut String,
    location_dirty: &mut bool,
    location_submitted: &mut bool,
    focus_location_field_for_search: bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    frame_intents: &mut Vec<GraphIntent>,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
) {
    let location_id = toolbar_location_input_id(command_bar_focus_target.active_pane());

    // Display mode: no active search session and field not focused from last frame.
    // Show Navigator breadcrumb. Clicking any token or the scope badge enters input mode.
    let field_has_focus = ctx.memory(|m| m.has_focus(location_id));
    let in_input_mode = field_has_focus || omnibar_search_session.is_some();

    if !in_input_mode {
        let available_width = ui.available_width().max(160.0);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available_width, LOCATION_INPUT_HEIGHT),
            egui::Sense::click(),
        );
        if ui.is_rect_visible(rect) {
            let visuals = ui.visuals();
            let bg = visuals.extreme_bg_color;
            let stroke = visuals.widgets.noninteractive.bg_stroke;
            ui.painter().rect(
                rect,
                egui::CornerRadius::same(4),
                bg,
                stroke,
                egui::StrokeKind::Inside,
            );

            let inner_margin = 6.0;
            let mut cursor_x = rect.left() + inner_margin;
            let text_y = rect.center().y;
            let font_id = egui::FontId::proportional(13.0);
            let text_color = visuals.text_color();
            let sep_color = visuals.weak_text_color();

            if let Some(breadcrumb) = &navigator_ctx.breadcrumb {
                for (i, token) in breadcrumb.tokens.iter().enumerate() {
                    if i > 0 {
                        let sep = " › ";
                        let sep_galley = ui.painter().layout_no_wrap(
                            sep.to_string(),
                            font_id.clone(),
                            sep_color,
                        );
                        ui.painter().galley(
                            egui::pos2(cursor_x, text_y - sep_galley.size().y / 2.0),
                            sep_galley.clone(),
                            sep_color,
                        );
                        cursor_x += sep_galley.size().x;
                    }
                    let galley = ui.painter().layout_no_wrap(
                        token.label.clone(),
                        font_id.clone(),
                        text_color,
                    );
                    ui.painter().galley(
                        egui::pos2(cursor_x, text_y - galley.size().y / 2.0),
                        galley.clone(),
                        text_color,
                    );
                    cursor_x += galley.size().x;
                }
            } else {
                let hint_galley = ui.painter().layout_no_wrap(
                    LOCATION_INPUT_HINT_TEXT.to_string(),
                    font_id.clone(),
                    sep_color,
                );
                ui.painter().galley(
                    egui::pos2(cursor_x, text_y - hint_galley.size().y / 2.0),
                    hint_galley,
                    sep_color,
                );
            }

            if let Some(badge) = &navigator_ctx.scope_badge {
                let badge_galley =
                    ui.painter()
                        .layout_no_wrap(badge.clone(), font_id.clone(), sep_color);
                let badge_x = rect.right() - inner_margin - badge_galley.size().x;
                if badge_x > cursor_x + 4.0 {
                    ui.painter().galley(
                        egui::pos2(badge_x, text_y - badge_galley.size().y / 2.0),
                        badge_galley,
                        sep_color,
                    );
                }
            }
        }
        if response.clicked() || focus_location_field_for_search {
            ctx.memory_mut(|m| m.request_focus(location_id));
        }
        if ui.input(|i| {
            if cfg!(target_os = "macos") {
                i.clone().consume_key(Modifiers::COMMAND, Key::L)
            } else {
                i.clone().consume_key(Modifiers::COMMAND, Key::L)
                    || i.clone().consume_key(Modifiers::ALT, Key::D)
            }
        }) {
            ctx.memory_mut(|m| m.request_focus(location_id));
            *local_widget_focus = Some(LocalFocusTarget::ToolbarLocation {
                pane_id: command_bar_focus_target.active_pane(),
            });
        }
        return;
    }

    let location_field = ui.add_sized(
        [ui.available_width().max(160.0), LOCATION_INPUT_HEIGHT],
        egui::TextEdit::singleline(location)
            .id(location_id)
            .hint_text(LOCATION_INPUT_HINT_TEXT),
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
        *local_widget_focus = Some(LocalFocusTarget::ToolbarLocation {
            pane_id: command_bar_focus_target.active_pane(),
        });
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
    if location_field.gained_focus() {
        *local_widget_focus = Some(LocalFocusTarget::ToolbarLocation {
            pane_id: command_bar_focus_target.active_pane(),
        });
    }

    if location_field.has_focus() {
        *local_widget_focus = Some(LocalFocusTarget::ToolbarLocation {
            pane_id: command_bar_focus_target.active_pane(),
        });
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
                        *omnibar_search_session = Some(search_provider_session(
                            provider,
                            trimmed_location,
                            vec![OmnibarMatch::SearchQuery {
                                query: query.to_string(),
                                provider,
                            }],
                            query,
                            true,
                        ));
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
                            has_node_panes,
                        );
                        *omnibar_search_session = if matches.is_empty() {
                            None
                        } else {
                            Some(OmnibarSearchSession::new_graph(mode, query, matches))
                        };
                    }
                }
            }
        } else if trimmed_location.len() >= OMNIBAR_PROVIDER_MIN_QUERY_LEN {
            let provider =
                default_search_provider_from_searchpage(&state.app_preferences.searchpage)
                    .unwrap_or(SearchProviderKind::DuckDuckGo);
            let (initial_matches, should_fetch_provider) = non_at_matches_for_settings(
                graph_app,
                tiles_tree,
                trimmed_location,
                has_node_panes,
            );
            let needs_refresh = !omnibar_search_session.as_ref().is_some_and(|session| {
                session.kind == OmnibarSessionKind::SearchProvider(provider)
                    && session.query == trimmed_location
            });
            if needs_refresh {
                *omnibar_search_session = Some(search_provider_session(
                    provider,
                    trimmed_location,
                    initial_matches,
                    trimmed_location,
                    should_fetch_provider,
                ));
            }
        } else if trimmed_location.is_empty() {
            let local_workspace_tab_matches = omnibar_matches_for_query(
                graph_app,
                tiles_tree,
                OmnibarSearchMode::TabsLocal,
                "",
                has_node_panes,
            );
            let provider =
                default_search_provider_from_searchpage(&state.app_preferences.searchpage)
                    .unwrap_or(SearchProviderKind::DuckDuckGo);
            *omnibar_search_session = Some(OmnibarSearchSession::new_search_provider(
                provider,
                String::new(),
                local_workspace_tab_matches,
                ProviderSuggestionMailbox::idle(),
            ));
        } else {
            *omnibar_search_session = None;
        }
    }
    if location_field.lost_focus()
        && matches!(
            *local_widget_focus,
            Some(LocalFocusTarget::ToolbarLocation { .. })
        )
    {
        *local_widget_focus = None;
    }

    if let Some(session) = omnibar_search_session.as_mut()
        && matches!(session.kind, OmnibarSessionKind::SearchProvider(_))
        && location_field.has_focus()
        && session.query == location.trim()
    {
        let mut fetched_outcome = None;
        if let Some(deadline) = session.provider_mailbox.debounce_deadline
            && session.provider_mailbox.rx.is_none()
            && Instant::now() >= deadline
            && let OmnibarSessionKind::SearchProvider(provider) = session.kind
        {
            session.provider_mailbox.debounce_deadline = None;
            let provider_query = provider_query_for_session(session);
            session.provider_mailbox.request_query = Some(provider_query.clone());
            let cache_key = provider_cache_key(provider, &provider_query);
            if let Some(cached_suggestions) = graph_app
                .workspace
                .graph_runtime
                .runtime_caches
                .get_suggestions(&cache_key)
            {
                fetched_outcome = Some(outcome_from_cached_suggestions(
                    provider,
                    &cached_suggestions,
                ));
            } else {
                emit_omnibar_provider_mailbox_request_started(&provider_query);
                session.provider_mailbox.rx = Some(spawn_provider_suggestion_request(
                    control_panel,
                    provider,
                    &provider_query,
                    graph_app.workspace.graph_runtime.runtime_caches.clone(),
                ));
            }
        }

        if let Some(rx) = &session.provider_mailbox.rx {
            match rx.try_recv() {
                Ok(outcome) => fetched_outcome = Some(outcome),
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(75));
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    fetched_outcome = Some(ProviderSuggestionFetchOutcome {
                        matches: Vec::new(),
                        status: ProviderSuggestionStatus::Failed(ProviderSuggestionError::Network),
                    });
                }
            }
        }
        if session.provider_mailbox.debounce_deadline.is_some() {
            ctx.request_repaint_after(Duration::from_millis(75));
        }
        if let Some(outcome) = fetched_outcome {
            session.provider_mailbox.rx = None;
            if !provider_query_matches_mailbox(session) {
                session.provider_mailbox.clear_pending();
                session.provider_mailbox.status = if session.matches.is_empty() {
                    ProviderSuggestionStatus::Idle
                } else {
                    ProviderSuggestionStatus::Ready
                };
                emit_omnibar_provider_mailbox_stale();
                return;
            }
            if let OmnibarSessionKind::SearchProvider(provider) = session.kind
                && matches!(outcome.status, ProviderSuggestionStatus::Ready)
            {
                let suggestions: Vec<String> = outcome
                    .matches
                    .iter()
                    .filter_map(|entry| match entry {
                        OmnibarMatch::SearchQuery {
                            query,
                            provider: entry_provider,
                        } if *entry_provider == provider => Some(query.clone()),
                        _ => None,
                    })
                    .collect();
                if !suggestions.is_empty() {
                    let provider_query = provider_query_for_session(session);
                    graph_app
                        .workspace
                        .graph_runtime
                        .runtime_caches
                        .insert_suggestions(
                            provider_cache_key(provider, &provider_query),
                            suggestions,
                        );
                }
            }
            session.provider_mailbox.status = outcome.status;
            if !session.query.starts_with('@') {
                let fallback_scope = if graph_app.workspace.chrome_ui.omnibar_preferred_scope
                    == OmnibarPreferredScope::ProviderDefault
                {
                    OmnibarPreferredScope::Auto
                } else {
                    graph_app.workspace.chrome_ui.omnibar_preferred_scope
                };
                let primary_matches = non_at_primary_matches_for_scope(
                    graph_app,
                    tiles_tree,
                    &session.query,
                    has_node_panes,
                    fallback_scope,
                );
                match graph_app.workspace.chrome_ui.omnibar_non_at_order {
                    OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal => {
                        session.matches.extend(outcome.matches);
                    }
                    OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => {
                        if outcome.matches.is_empty() {
                            session.matches = primary_matches;
                        } else {
                            session.matches = outcome.matches;
                            session.matches.extend(primary_matches);
                        }
                    }
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
                    has_node_panes,
                );
            }
            if !session.matches.is_empty()
                && !matches!(session.provider_mailbox.status, ProviderSuggestionStatus::Failed(_))
            {
                session.provider_mailbox.status = ProviderSuggestionStatus::Ready;
            }
            let mailbox_failed = matches!(
                session.provider_mailbox.status,
                ProviderSuggestionStatus::Failed(_)
            );
            session.provider_mailbox.clear_pending();
            if mailbox_failed {
                emit_omnibar_provider_mailbox_failed();
            } else {
                emit_omnibar_provider_mailbox_applied();
            }
            session.active_index = session
                .active_index
                .min(session.matches.len().saturating_sub(1));
        }
    }

    // Delegate dropdown rendering to focused helper module
    let mut retain_omnibar_focus = toolbar_location_dropdown::render_omnibar_dropdown(
        ctx,
        ui,
        &location_field,
        location,
        location_dirty,
        omnibar_search_session,
        graph_app,
        tiles_tree,
        is_graph_view,
        command_bar_focus_target,
        window,
        has_node_panes,
        frame_intents,
        open_selected_mode_after_submit,
    );

    let enter_while_focused = location_field.has_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let enter_after_focus_loss =
        location_field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if enter_while_focused {
        *location_submitted = true;
    }
    if should_dispatch_location_submit(
        enter_while_focused,
        *location_submitted,
        enter_after_focus_loss,
    ) {
        retain_omnibar_focus |= super::toolbar_location_submit::handle_location_submit(
            ui,
            state,
            graph_app,
            window,
            tiles_tree,
            command_bar_focus_target,
            has_node_panes,
            is_graph_view,
            location,
            location_dirty,
            location_submitted,
            omnibar_search_session,
            frame_intents,
            open_selected_mode_after_submit,
        );
    }

    if retain_omnibar_focus {
        ctx.memory_mut(|memory| memory.request_focus(location_id));
        *local_widget_focus = Some(LocalFocusTarget::ToolbarLocation {
            pane_id: command_bar_focus_target.active_pane(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LOCATION_INPUT_HINT_TEXT, OmnibarSearchSession, ProviderSuggestionMailbox,
        SearchProviderKind, provider_cache_key, provider_query_for_session,
        provider_query_matches_mailbox, should_dispatch_location_submit,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn submit_dispatch_triggers_for_focused_enter() {
        assert!(should_dispatch_location_submit(true, false, false));
    }

    #[test]
    fn submit_dispatch_triggers_for_queued_submit() {
        assert!(should_dispatch_location_submit(false, true, false));
    }

    #[test]
    fn submit_dispatch_ignores_enter_after_focus_loss() {
        assert!(!should_dispatch_location_submit(false, false, true));
    }

    #[test]
    fn submit_dispatch_triggers_for_queued_submit_after_focus_loss() {
        assert!(should_dispatch_location_submit(false, true, true));
    }

    #[test]
    fn submit_dispatch_does_not_trigger_without_enter_or_queue() {
        assert!(!should_dispatch_location_submit(false, false, false));
    }

    #[test]
    fn location_input_hint_text_provides_search_and_address_instruction() {
        assert!(LOCATION_INPUT_HINT_TEXT.contains("Search"));
        assert!(LOCATION_INPUT_HINT_TEXT.contains("address"));
    }

    #[test]
    fn provider_cache_key_namespaces_provider_and_query() {
        assert_eq!(
            provider_cache_key(SearchProviderKind::Google, "rust"),
            "provider:google:rust"
        );
    }

    #[test]
    fn provider_query_for_session_strips_at_provider_prefix() {
        let session = OmnibarSearchSession::new_search_provider(
            SearchProviderKind::DuckDuckGo,
            "@d rust async",
            Vec::new(),
            ProviderSuggestionMailbox::idle(),
        );
        assert_eq!(provider_query_for_session(&session), "rust async");
    }

    #[test]
    fn provider_query_for_session_keeps_plain_query_for_non_at_mode() {
        let session = OmnibarSearchSession::new_search_provider(
            SearchProviderKind::Bing,
            "plain query",
            Vec::new(),
            ProviderSuggestionMailbox::idle(),
        );
        assert_eq!(provider_query_for_session(&session), "plain query");
    }

    #[test]
    fn provider_query_for_session_falls_back_when_provider_token_invalid() {
        let session = OmnibarSearchSession::new_search_provider(
            SearchProviderKind::Google,
            "@x raw",
            Vec::new(),
            ProviderSuggestionMailbox::idle(),
        );
        assert_eq!(provider_query_for_session(&session), "@x raw");
    }

    #[test]
    fn provider_query_matches_mailbox_when_request_query_matches_session_query() {
        let session = OmnibarSearchSession::new_search_provider(
            SearchProviderKind::DuckDuckGo,
            "@d rust async",
            Vec::new(),
            ProviderSuggestionMailbox::debounced(
                "rust async".to_string(),
                Instant::now() + Duration::from_millis(10),
            ),
        );

        assert!(provider_query_matches_mailbox(&session));
    }

    #[test]
    fn provider_query_mismatch_marks_mailbox_as_stale() {
        let session = OmnibarSearchSession::new_search_provider(
            SearchProviderKind::DuckDuckGo,
            "@d rust async",
            Vec::new(),
            ProviderSuggestionMailbox::debounced(
                "rust book".to_string(),
                Instant::now() + Duration::from_millis(10),
            ),
        );

        assert!(!provider_query_matches_mailbox(&session));
    }
}
