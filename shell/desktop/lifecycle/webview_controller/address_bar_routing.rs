use super::*;

pub(super) fn resolve_detail_submit_target(
    app: &GraphBrowserApp,
    focused_node: Option<NodeKey>,
    preferred_webview: Option<WebViewId>,
) -> (Option<NodeKey>, Option<WebViewId>) {
    if let Some(node_key) = focused_node {
        return (Some(node_key), app.get_webview_for_node(node_key));
    }

    if let Some(webview_id) = preferred_webview {
        return (app.get_node_for_webview(webview_id), Some(webview_id));
    }

    (None, None)
}

pub(super) fn workbench_route_intent_for_verso_url(
    normalized_url: &str,
) -> Option<WorkbenchIntent> {
    let parsed = VersoAddress::parse(normalized_url)?;
    let canonical_url = parsed.to_string();
    match parsed {
        VersoAddress::Settings(_) => Some(WorkbenchIntent::OpenSettingsUrl { url: canonical_url }),
        VersoAddress::Frame(_) => Some(WorkbenchIntent::OpenFrameUrl { url: canonical_url }),
        VersoAddress::TileGroup(_) => None,
        VersoAddress::Tool { .. } => Some(WorkbenchIntent::OpenToolUrl { url: canonical_url }),
        VersoAddress::View(_) => Some(WorkbenchIntent::OpenViewUrl { url: canonical_url }),
        VersoAddress::Clip(_) => Some(WorkbenchIntent::OpenClipUrl { url: canonical_url }),
        VersoAddress::Other { .. } => None,
    }
}

pub(super) fn route_intent_for_internal_or_domain_url(
    normalized_url: &str,
) -> Option<WorkbenchIntent> {
    if let Some(intent) = workbench_route_intent_for_verso_url(normalized_url) {
        return Some(intent);
    }

    if let Some(address) = NoteAddress::parse(normalized_url) {
        return Some(WorkbenchIntent::OpenNoteUrl {
            url: address.to_string(),
        });
    }

    if let Some(address) = NodeAddress::parse(normalized_url) {
        return Some(WorkbenchIntent::OpenNodeUrl {
            url: address.to_string(),
        });
    }

    if let Some(address) = GraphAddress::parse(normalized_url) {
        return Some(WorkbenchIntent::OpenGraphUrl {
            url: address.to_string(),
        });
    }

    None
}

pub(super) fn handle_address_bar_submit_intents(
    app: &GraphBrowserApp,
    url: &str,
    is_graph_view: bool,
    focused_node: Option<NodeKey>,
    window: &EmbedderWindow,
    searchpage: &str,
) -> AddressBarIntentOutcome {
    let input = url.trim();
    if let Some(query) = input.strip_prefix('@') {
        let intents = registries::phase2_execute_omnibox_node_search_action(app, query);

        return AddressBarIntentOutcome {
            outcome: AddressBarSubmitOutcome {
                mark_clean: true,
                open_selected_tile: false,
            },
            intents,
            workbench_intents: Vec::new(),
        };
    }

    if is_graph_view {
        let (normalized_input, workbench_intent) =
            match location_bar_input_to_url(input, searchpage) {
                Some(parsed_url) => {
                    let decision = registries::phase0_decide_navigation_with_control(
                        parsed_url,
                        None,
                        registries::protocol::ProtocolResolveControl::default(),
                    );
                    let Some(decision) = decision else {
                        return AddressBarIntentOutcome {
                            outcome: AddressBarSubmitOutcome {
                                mark_clean: false,
                                open_selected_tile: false,
                            },
                            intents: Vec::new(),
                            workbench_intents: Vec::new(),
                        };
                    };
                    (
                        decision.normalized_url.as_str().to_string(),
                        route_intent_for_internal_or_domain_url(decision.normalized_url.as_str()),
                    )
                }
                None => (input.to_string(), None),
            };
        if let Some(workbench_intent) = workbench_intent {
            return AddressBarIntentOutcome {
                outcome: AddressBarSubmitOutcome {
                    mark_clean: true,
                    open_selected_tile: false,
                },
                intents: Vec::new(),
                workbench_intents: vec![workbench_intent],
            };
        }
        let (open_selected_tile, intents) = graph_intents_from_graph_view_submit_result(
            registries::phase2_execute_graph_view_submit_action(app, &normalized_input),
        );

        AddressBarIntentOutcome {
            outcome: AddressBarSubmitOutcome {
                mark_clean: true,
                open_selected_tile,
            },
            intents,
            workbench_intents: Vec::new(),
        }
    } else {
        let Some(parsed_url) = location_bar_input_to_url(input, searchpage) else {
            log::warn!("Failed to parse location: {}", input);
            return AddressBarIntentOutcome {
                outcome: AddressBarSubmitOutcome {
                    mark_clean: false,
                    open_selected_tile: false,
                },
                intents: Vec::new(),
                workbench_intents: Vec::new(),
            };
        };

        let (parsed_url, selected_viewer_id, viewer_surface, workbench_intent) = {
            let decision = registries::phase0_decide_navigation_with_control(
                parsed_url,
                None,
                registries::protocol::ProtocolResolveControl::default(),
            );
            let Some(decision) = decision else {
                return AddressBarIntentOutcome {
                    outcome: AddressBarSubmitOutcome {
                        mark_clean: false,
                        open_selected_tile: false,
                    },
                    intents: Vec::new(),
                    workbench_intents: Vec::new(),
                };
            };
            let normalized_url_string = decision.normalized_url.as_str().to_string();
            let selected_viewer_id = decision.viewer.viewer_id.to_string();
            let viewer_surface =
                registries::phase3_resolve_viewer_surface_profile(decision.viewer.viewer_id);
            (
                decision.normalized_url,
                selected_viewer_id,
                viewer_surface,
                route_intent_for_internal_or_domain_url(normalized_url_string.as_str()),
            )
        };

        if let Some(workbench_intent) = workbench_intent {
            return AddressBarIntentOutcome {
                outcome: AddressBarSubmitOutcome {
                    mark_clean: true,
                    open_selected_tile: false,
                },
                intents: Vec::new(),
                workbench_intents: vec![workbench_intent],
            };
        }

        if selected_viewer_id != "viewer:webview" {
            log::debug!(
                "viewer '{}' selected for '{}'; applying viewer surface '{}' (reader_mode_default={}, smooth_scroll_enabled={}, zoom_step={})",
                selected_viewer_id,
                parsed_url,
                viewer_surface.resolved_id,
                viewer_surface.profile.reader_mode_default,
                viewer_surface.profile.smooth_scroll_enabled,
                viewer_surface.profile.zoom_step
            );

            let (open_selected_tile, intents) = graph_intents_from_detail_submit_result(
                registries::phase2_execute_detail_view_submit_action(
                    app,
                    parsed_url.as_str(),
                    focused_node,
                ),
            );
            return AddressBarIntentOutcome {
                outcome: AddressBarSubmitOutcome {
                    mark_clean: true,
                    open_selected_tile,
                },
                intents,
                workbench_intents: Vec::new(),
            };
        }

        let preferred_input_webview = app.embedded_content_focus_webview();
        let (target_node, target_webview) =
            resolve_detail_submit_target(app, focused_node, preferred_input_webview);

        if let Some(webview_id) = target_webview
            && let Some(webview) = window.webview_by_id(webview_id)
        {
            window.retarget_input_to_webview(webview_id);
            webview.load(parsed_url.into_url());
            window.set_needs_update();
            return AddressBarIntentOutcome {
                outcome: AddressBarSubmitOutcome {
                    mark_clean: false,
                    open_selected_tile: false,
                },
                intents: Vec::new(),
                workbench_intents: Vec::new(),
            };
        }

        let (open_selected_tile, intents) = graph_intents_from_detail_submit_result(
            registries::phase2_execute_detail_view_submit_action(
                app,
                parsed_url.as_str(),
                target_node,
            ),
        );
        AddressBarIntentOutcome {
            outcome: AddressBarSubmitOutcome {
                mark_clean: true,
                open_selected_tile,
            },
            intents,
            workbench_intents: Vec::new(),
        }
    }
}
