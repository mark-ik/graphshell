use super::*;
use crate::shell::desktop::ui::persistence_ops;

pub(super) fn parse_omnibar_search_query(raw: &str) -> (OmnibarSearchMode, &str) {
    let trimmed = raw.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or_default();
    let tail = parts.next().unwrap_or_default().trim();
    if head == "T" {
        return (OmnibarSearchMode::TabsAll, tail);
    }
    if head.eq_ignore_ascii_case("t") || head.eq_ignore_ascii_case("tab") {
        return (OmnibarSearchMode::TabsLocal, tail);
    }
    if head == "N" {
        return (OmnibarSearchMode::NodesAll, tail);
    }
    if head.eq_ignore_ascii_case("n") || head.eq_ignore_ascii_case("node") {
        return (OmnibarSearchMode::NodesLocal, tail);
    }
    if head == "E" {
        return (OmnibarSearchMode::EdgesAll, tail);
    }
    if head.eq_ignore_ascii_case("e") || head.eq_ignore_ascii_case("edge") {
        return (OmnibarSearchMode::EdgesLocal, tail);
    }
    (OmnibarSearchMode::Mixed, trimmed)
}

pub(super) fn parse_provider_search_query(raw: &str) -> Option<(SearchProviderKind, &str)> {
    let trimmed = raw.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or_default();
    let tail = parts.next().unwrap_or_default().trim();
    let provider = if head.eq_ignore_ascii_case("g") || head.eq_ignore_ascii_case("google") {
        SearchProviderKind::Google
    } else if head.eq_ignore_ascii_case("b") || head.eq_ignore_ascii_case("bing") {
        SearchProviderKind::Bing
    } else if head.eq_ignore_ascii_case("d")
        || head.eq_ignore_ascii_case("ddg")
        || head.eq_ignore_ascii_case("duckduckgo")
    {
        SearchProviderKind::DuckDuckGo
    } else {
        return None;
    };
    Some((provider, tail))
}

fn omnibar_import_search_text(graph_app: &GraphBrowserApp, key: NodeKey) -> String {
    graph_app
        .domain_graph()
        .import_record_summaries_for_node(key)
        .into_iter()
        .map(|record| {
            format!(
                "{} {} {} {}",
                record.source_label,
                record.source_id,
                record.record_id,
                crate::graph::format_imported_at_secs(record.imported_at_secs),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn omnibar_import_label_suffix(graph_app: &GraphBrowserApp, key: NodeKey) -> String {
    let import_records = graph_app
        .domain_graph()
        .import_record_summaries_for_node(key);
    let Some(primary_record) = import_records.first() else {
        return String::new();
    };
    let extra_count = import_records.len().saturating_sub(1);
    let extra_suffix = if extra_count == 0 {
        String::new()
    } else {
        format!(" +{extra_count}")
    };
    format!(
        "  [Imported: {} {} {}{}]",
        primary_record.source_label,
        crate::graph::format_imported_at_secs(primary_record.imported_at_secs),
        primary_record.record_id,
        extra_suffix,
    )
}

pub(super) fn default_search_provider_from_searchpage(
    searchpage: &str,
) -> Option<SearchProviderKind> {
    let host = url::Url::parse(searchpage)
        .ok()?
        .host_str()?
        .to_ascii_lowercase();
    if host.contains("duckduckgo.") {
        return Some(SearchProviderKind::DuckDuckGo);
    }
    if host.contains("bing.") {
        return Some(SearchProviderKind::Bing);
    }
    if host.contains("google.") {
        return Some(SearchProviderKind::Google);
    }
    None
}

pub(super) fn searchpage_template_for_provider(provider: SearchProviderKind) -> &'static str {
    match provider {
        SearchProviderKind::DuckDuckGo => "https://duckduckgo.com/html/?q=%s",
        SearchProviderKind::Bing => "https://www.bing.com/search?q=%s",
        SearchProviderKind::Google => "https://www.google.com/search?q=%s",
    }
}

pub(super) fn spawn_provider_suggestion_request(
    provider: SearchProviderKind,
    query: &str,
    runtime_caches: crate::shell::desktop::runtime::caches::RuntimeCaches,
) -> Receiver<ProviderSuggestionFetchOutcome> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let query = query.to_string();
    thread::spawn(move || {
        let outcome = match fetch_provider_search_suggestions(provider, &query, &runtime_caches) {
            Ok(suggestions) => ProviderSuggestionFetchOutcome {
                matches: suggestions
                    .into_iter()
                    .map(|query| OmnibarMatch::SearchQuery { query, provider })
                    .collect(),
                status: ProviderSuggestionStatus::Ready,
            },
            Err(error) => ProviderSuggestionFetchOutcome {
                matches: Vec::new(),
                status: ProviderSuggestionStatus::Failed(error),
            },
        };
        let _ = tx.send(outcome);
    });
    rx
}

fn fetch_provider_search_suggestions(
    provider: SearchProviderKind,
    query: &str,
    runtime_caches: &crate::shell::desktop::runtime::caches::RuntimeCaches,
) -> Result<Vec<String>, ProviderSuggestionError> {
    let parsed_cache_key = provider_parsed_metadata_cache_key(provider, query);
    if let Some(cached_value) = runtime_caches.get_parsed_metadata(&parsed_cache_key)
        && let Some(suggestions) = parse_provider_suggestion_value(cached_value.as_ref(), query)
    {
        return Ok(suggestions);
    }

    let suggest_url = provider_suggest_url(provider, query);
    let body = match router::fetch_text(&suggest_url) {
        Ok(body) => body,
        Err(OutboundFetchError::HttpStatus(status)) => {
            return Err(ProviderSuggestionError::HttpStatus(status));
        }
        Err(
            OutboundFetchError::Network
            | OutboundFetchError::InvalidUrl
            | OutboundFetchError::UnsupportedScheme,
        ) => return Err(ProviderSuggestionError::Network),
        Err(OutboundFetchError::Body) => return Err(ProviderSuggestionError::Parse),
    };
    let parsed_value =
        serde_json::from_str::<Value>(&body).map_err(|_| ProviderSuggestionError::Parse)?;
    runtime_caches.insert_parsed_metadata(parsed_cache_key, parsed_value.clone());
    parse_provider_suggestion_value(&parsed_value, query).ok_or(ProviderSuggestionError::Parse)
}

fn provider_parsed_metadata_cache_key(provider: SearchProviderKind, query: &str) -> String {
    let provider_key = match provider {
        SearchProviderKind::DuckDuckGo => "duckduckgo",
        SearchProviderKind::Bing => "bing",
        SearchProviderKind::Google => "google",
    };
    format!(
        "provider:parsed_suggestions:{provider_key}:{}",
        query.trim()
    )
}

fn provider_suggest_url(provider: SearchProviderKind, query: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    match provider {
        SearchProviderKind::DuckDuckGo => {
            format!("https://duckduckgo.com/ac/?q={encoded}&type=list")
        }
        SearchProviderKind::Bing => format!("https://api.bing.com/osjson.aspx?query={encoded}"),
        SearchProviderKind::Google => {
            format!("https://suggestqueries.google.com/complete/search?client=firefox&q={encoded}")
        }
    }
}

fn parse_provider_suggestion_body(body: &str, fallback_query: &str) -> Option<Vec<String>> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return None;
    };
    parse_provider_suggestion_value(&value, fallback_query)
}

fn parse_provider_suggestion_value(value: &Value, fallback_query: &str) -> Option<Vec<String>> {
    let mut suggestions = Vec::new();

    if let Some(items) = value.as_array() {
        if let Some(second) = items.get(1).and_then(Value::as_array) {
            for item in second {
                if let Some(s) = item.as_str() {
                    suggestions.push(s.to_string());
                }
            }
        } else {
            for item in items {
                if let Some(s) = item.get("phrase").and_then(Value::as_str) {
                    suggestions.push(s.to_string());
                }
            }
        }
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    if seen.insert(fallback_query.to_string()) {
        deduped.push(fallback_query.to_string());
    }
    for suggestion in suggestions {
        let normalized = suggestion.trim();
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.to_string()) {
            deduped.push(normalized.to_string());
        }
    }
    Some(deduped)
}

fn connected_nodes_matches_for_query(
    graph_app: &mut GraphBrowserApp,
    query: &str,
    exclude: &HashSet<NodeKey>,
) -> Vec<OmnibarMatch> {
    let Some(context) = graph_app.focused_selection().primary() else {
        return Vec::new();
    };
    let hop_distances = graph_app.cached_hop_distances_for_context(context);
    let ranked = fuzzy_match_node_keys(graph_app.domain_graph(), query);
    let rank_index: HashMap<NodeKey, usize> = ranked
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, key)| (key, idx))
        .collect();

    let mut connected: Vec<NodeKey> = ranked
        .into_iter()
        .filter(|key| !exclude.contains(key))
        .filter(|key| hop_distances.contains_key(key))
        .collect();
    connected.sort_by_key(|key| {
        (
            hop_distances.get(key).copied().unwrap_or(usize::MAX),
            rank_index.get(key).copied().unwrap_or(usize::MAX),
        )
    });
    connected
        .into_iter()
        .take(OMNIBAR_CONNECTED_NON_AT_CAP)
        .map(OmnibarMatch::Node)
        .collect()
}

fn non_at_contextual_matches(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    query: &str,
    has_node_panes: bool,
) -> Vec<OmnibarMatch> {
    let local_tabs = omnibar_matches_for_query(
        graph_app,
        tiles_tree,
        OmnibarSearchMode::TabsLocal,
        query,
        has_node_panes,
    );
    let local_tab_keys: HashSet<NodeKey> = local_tabs
        .iter()
        .filter_map(|m| match m {
            OmnibarMatch::Node(key) => Some(*key),
            _ => None,
        })
        .collect();
    let mut out = local_tabs;
    out.extend(connected_nodes_matches_for_query(
        graph_app,
        query,
        &local_tab_keys,
    ));
    dedupe_matches_in_order(out)
}

pub(super) fn non_at_primary_matches_for_scope(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    query: &str,
    has_node_panes: bool,
    scope: OmnibarPreferredScope,
) -> Vec<OmnibarMatch> {
    match scope {
        OmnibarPreferredScope::Auto => {
            non_at_contextual_matches(graph_app, tiles_tree, query, has_node_panes)
        }
        OmnibarPreferredScope::LocalTabs => omnibar_matches_for_query(
            graph_app,
            tiles_tree,
            OmnibarSearchMode::TabsLocal,
            query,
            has_node_panes,
        ),
        OmnibarPreferredScope::ConnectedNodes => {
            connected_nodes_matches_for_query(graph_app, query, &HashSet::new())
        }
        OmnibarPreferredScope::ProviderDefault => Vec::new(),
        OmnibarPreferredScope::GlobalNodes => omnibar_matches_for_query(
            graph_app,
            tiles_tree,
            OmnibarSearchMode::NodesAll,
            query,
            has_node_panes,
        )
        .into_iter()
        .take(OMNIBAR_GLOBAL_NODES_FALLBACK_CAP)
        .collect(),
        OmnibarPreferredScope::GlobalTabs => omnibar_matches_for_query(
            graph_app,
            tiles_tree,
            OmnibarSearchMode::TabsAll,
            query,
            has_node_panes,
        )
        .into_iter()
        .take(OMNIBAR_GLOBAL_TABS_FALLBACK_CAP)
        .collect(),
    }
}

pub(super) fn non_at_matches_for_settings(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    query: &str,
    has_node_panes: bool,
) -> (Vec<OmnibarMatch>, bool) {
    let primary_matches = non_at_primary_matches_for_scope(
        graph_app,
        tiles_tree,
        query,
        has_node_panes,
        graph_app.workspace.chrome_ui.omnibar_preferred_scope,
    );

    match graph_app.workspace.chrome_ui.omnibar_non_at_order {
        OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal => {
            if primary_matches.is_empty()
                || graph_app.workspace.chrome_ui.omnibar_preferred_scope
                    == OmnibarPreferredScope::ProviderDefault
            {
                (primary_matches, true)
            } else {
                (primary_matches, false)
            }
        }
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => (Vec::new(), true),
    }
}

pub(super) fn non_at_global_fallback_matches(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    query: &str,
    has_node_panes: bool,
) -> Vec<OmnibarMatch> {
    let mut out = Vec::new();
    out.extend(
        omnibar_matches_for_query(
            graph_app,
            tiles_tree,
            OmnibarSearchMode::NodesAll,
            query,
            has_node_panes,
        )
        .into_iter()
        .take(OMNIBAR_GLOBAL_NODES_FALLBACK_CAP),
    );
    out.extend(
        omnibar_matches_for_query(
            graph_app,
            tiles_tree,
            OmnibarSearchMode::TabsAll,
            query,
            has_node_panes,
        )
        .into_iter()
        .take(OMNIBAR_GLOBAL_TABS_FALLBACK_CAP),
    );
    dedupe_matches_in_order(out)
}

fn tab_node_keys_in_tree(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
    tile_grouping::webview_tab_group_memberships(tiles_tree)
        .keys()
        .copied()
        .collect()
}

fn saved_tab_node_keys(graph_app: &GraphBrowserApp) -> HashSet<NodeKey> {
    let mut saved_tab_nodes = HashSet::new();
    for frame_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&frame_name) {
            continue;
        }
        let Ok(bundle) = persistence_ops::load_named_frame_bundle(graph_app, &frame_name) else {
            continue;
        };
        if let Ok((tree, _)) =
            persistence_ops::restore_runtime_tree_from_frame_bundle(graph_app, &bundle)
        {
            saved_tab_nodes.extend(tab_node_keys_in_tree(&tree));
        }
    }
    saved_tab_nodes
}

fn edge_type_label(edge_type: crate::graph::EdgeType) -> &'static str {
    match edge_type {
        crate::graph::EdgeType::Hyperlink => "hyperlink",
        crate::graph::EdgeType::History => "history",
        crate::graph::EdgeType::UserGrouped => "user_grouped",
        crate::graph::EdgeType::ArrangementRelation(sub_kind) => sub_kind.as_tag(),
        crate::graph::EdgeType::ContainmentRelation(sub_kind) => sub_kind.as_tag(),
        crate::graph::EdgeType::ImportedRelation => "imported_relation",
        crate::graph::EdgeType::AgentDerived { .. } => "agent_derived",
    }
}

pub(super) fn graph_center_for_new_node(graph_app: &GraphBrowserApp) -> Point2D<f32> {
    graph_app
        .domain_graph()
        .projected_centroid()
        .unwrap_or_else(|| Point2D::new(0.0, 0.0))
}

fn edge_candidates_for_graph(
    graph: &crate::graph::Graph,
    only_targets: Option<&HashSet<NodeKey>>,
) -> Vec<OmnibarSearchCandidate> {
    let mut out = Vec::new();
    for edge in graph.edges() {
        if let Some(filter) = only_targets
            && (!filter.contains(&edge.from) || !filter.contains(&edge.to))
        {
            continue;
        }
        let Some(from_node) = graph.get_node(edge.from) else {
            continue;
        };
        let Some(to_node) = graph.get_node(edge.to) else {
            continue;
        };
        out.push(OmnibarSearchCandidate {
            text: format!(
                "{} {} {} {} {}",
                edge_type_label(edge.edge_type),
                from_node.title,
                from_node.url,
                to_node.title,
                to_node.url
            ),
            target: OmnibarMatch::Edge {
                from: edge.from,
                to: edge.to,
            },
        });
    }
    out
}

fn node_candidates_for_graph(graph_app: &GraphBrowserApp) -> Vec<OmnibarSearchCandidate> {
    graph_app
        .domain_graph()
        .nodes()
        .map(|(key, node)| OmnibarSearchCandidate {
            text: format!(
                "{} {} {}",
                node.title,
                node.url,
                omnibar_import_search_text(graph_app, key)
            ),
            target: OmnibarMatch::Node(key),
        })
        .collect()
}

fn tab_candidates_for_keys(
    graph_app: &GraphBrowserApp,
    keys: &HashSet<NodeKey>,
) -> Vec<OmnibarSearchCandidate> {
    keys.iter()
        .filter_map(|key| {
            graph_app
                .domain_graph()
                .get_node(*key)
                .map(|node| OmnibarSearchCandidate {
                    text: format!(
                        "{} {} {}",
                        node.title,
                        node.url,
                        omnibar_import_search_text(graph_app, *key)
                    ),
                    target: OmnibarMatch::Node(*key),
                })
        })
        .collect()
}

pub(super) fn omnibar_match_signifier(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    m: &OmnibarMatch,
) -> &'static str {
    match m {
        OmnibarMatch::Node(key) => {
            let local_tabs = tab_node_keys_in_tree(tiles_tree);
            let saved_tabs = saved_tab_node_keys(graph_app);
            let is_local_tab = local_tabs.contains(key);
            let is_saved_tab = saved_tabs.contains(key);
            let is_connected = graph_app
                .focused_selection()
                .primary()
                .map(|context| graph_app.cached_hop_distances_for_context(context))
                .and_then(|hops| hops.get(key).copied())
                .unwrap_or(usize::MAX)
                != usize::MAX;
            if is_connected && is_local_tab {
                "related tab"
            } else if is_local_tab {
                "frame tab"
            } else if is_saved_tab {
                "other frame"
            } else if is_connected {
                "related node"
            } else {
                "graph node"
            }
        }
        OmnibarMatch::NodeUrl(_) => "historical",
        OmnibarMatch::SearchQuery { provider, .. } => match provider {
            SearchProviderKind::DuckDuckGo => "duckduckgo suggestion",
            SearchProviderKind::Bing => "bing suggestion",
            SearchProviderKind::Google => "google suggestion",
        },
        OmnibarMatch::Edge { .. } => "edge",
    }
}

pub(super) fn omnibar_match_label(graph_app: &GraphBrowserApp, m: &OmnibarMatch) -> String {
    match m {
        OmnibarMatch::Node(key) => graph_app
            .domain_graph()
            .get_node(*key)
            .map(|node| {
                format!(
                    "{}  {}{}",
                    node.title,
                    node.url,
                    omnibar_import_label_suffix(graph_app, *key)
                )
            })
            .unwrap_or_else(|| format!("node {}", key.index())),
        OmnibarMatch::NodeUrl(url) => url.clone(),
        OmnibarMatch::SearchQuery { query, .. } => query.clone(),
        OmnibarMatch::Edge { from, to } => {
            let from_label = graph_app
                .domain_graph()
                .get_node(*from)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| from.index().to_string());
            let to_label = graph_app
                .domain_graph()
                .get_node(*to)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| to.index().to_string());
            format!("{from_label} -> {to_label}")
        }
    }
}

pub(super) fn dedupe_matches_in_order(matches: Vec<OmnibarMatch>) -> Vec<OmnibarMatch> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in matches {
        if seen.insert(m.clone()) {
            out.push(m);
        }
    }
    out
}

fn ranked_matches(candidates: Vec<OmnibarSearchCandidate>, query: &str) -> Vec<OmnibarMatch> {
    dedupe_matches_in_order(
        fuzzy_match_items(candidates, query)
            .into_iter()
            .map(|candidate| candidate.target)
            .collect(),
    )
}

pub(super) fn apply_omnibar_match(
    graph_app: &GraphBrowserApp,
    active_match: OmnibarMatch,
    has_node_panes: bool,
    force_original_frame: bool,
    frame_intents: &mut Vec<GraphIntent>,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
) {
    match active_match {
        OmnibarMatch::Node(key) => {
            frame_intents.push(GraphIntent::ClearHighlightedEdge);
            if has_node_panes && force_original_frame {
                frame_intents.push(GraphIntent::OpenNodeFrameRouted {
                    key,
                    prefer_frame: None,
                });
            } else {
                frame_intents.push(GraphIntent::SelectNode {
                    key,
                    multi_select: false,
                });
                if has_node_panes {
                    *open_selected_mode_after_submit = Some(ToolbarOpenMode::Tab);
                }
            }
        }
        OmnibarMatch::NodeUrl(url) => {
            frame_intents.push(GraphIntent::ClearHighlightedEdge);
            if let Some((key, _)) = graph_app.domain_graph().get_node_by_url(&url) {
                if has_node_panes {
                    frame_intents.push(GraphIntent::OpenNodeFrameRouted {
                        key,
                        prefer_frame: None,
                    });
                } else {
                    frame_intents.push(GraphIntent::SelectNode {
                        key,
                        multi_select: false,
                    });
                }
            } else {
                if has_node_panes {
                    frame_intents.push(GraphIntent::CreateNodeAtUrlAndOpen {
                        url,
                        position: graph_center_for_new_node(graph_app),
                        mode: PendingTileOpenMode::Tab,
                    });
                } else {
                    frame_intents.push(GraphIntent::CreateNodeAtUrl {
                        url,
                        position: graph_center_for_new_node(graph_app),
                    });
                }
            }
        }
        OmnibarMatch::SearchQuery { .. } => {}
        OmnibarMatch::Edge { from, to } => {
            frame_intents.push(GraphIntent::SetHighlightedEdge { from, to });
            frame_intents.push(GraphIntent::SelectNode {
                key: from,
                multi_select: false,
            });
            frame_intents.push(GraphIntent::SelectNode {
                key: to,
                multi_select: true,
            });
        }
    }
}

pub(super) fn omnibar_matches_for_query(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    mode: OmnibarSearchMode,
    query: &str,
    has_node_panes: bool,
) -> Vec<OmnibarMatch> {
    let query = query.trim();
    if query.is_empty() {
        if matches!(mode, OmnibarSearchMode::TabsLocal) {
            let mut local_tabs: Vec<NodeKey> =
                tab_node_keys_in_tree(tiles_tree).into_iter().collect();
            local_tabs.sort_by_key(|key| key.index());
            return local_tabs.into_iter().map(OmnibarMatch::Node).collect();
        }
        return Vec::new();
    }

    let local_tab_nodes = tab_node_keys_in_tree(tiles_tree);
    let local_node_candidates = node_candidates_for_graph(graph_app);
    let local_edge_candidates = edge_candidates_for_graph(graph_app.domain_graph(), None);

    let saved_tab_nodes = saved_tab_node_keys(graph_app);

    let mut all_graph_node_candidates = local_node_candidates.clone();
    let mut all_graph_edge_candidates = local_edge_candidates.clone();
    let mut node_urls_seen: HashSet<String> = graph_app
        .domain_graph()
        .nodes()
        .map(|(_, node)| node.url.clone())
        .collect();
    let mut mapped_edge_keys_seen: HashSet<(NodeKey, NodeKey)> = graph_app
        .domain_graph()
        .edges()
        .map(|e| (e.from, e.to))
        .collect();

    if let Some(snapshot) = graph_app.peek_latest_graph_snapshot() {
        for (_, node) in snapshot.nodes() {
            if node_urls_seen.insert(node.url.clone()) {
                all_graph_node_candidates.push(OmnibarSearchCandidate {
                    text: format!("{} {}", node.title, node.url),
                    target: OmnibarMatch::NodeUrl(node.url.clone()),
                });
            }
        }
        for edge in snapshot.edges() {
            let Some(from_node) = snapshot.get_node(edge.from) else {
                continue;
            };
            let Some(to_node) = snapshot.get_node(edge.to) else {
                continue;
            };
            let current_from = graph_app
                .domain_graph()
                .get_node_by_url(&from_node.url)
                .map(|(k, _)| k);
            let current_to = graph_app
                .domain_graph()
                .get_node_by_url(&to_node.url)
                .map(|(k, _)| k);
            if let (Some(from_key), Some(to_key)) = (current_from, current_to)
                && mapped_edge_keys_seen.insert((from_key, to_key))
            {
                all_graph_edge_candidates.push(OmnibarSearchCandidate {
                    text: format!(
                        "{} {} {} {} {}",
                        edge_type_label(edge.edge_type),
                        from_node.title,
                        from_node.url,
                        to_node.title,
                        to_node.url
                    ),
                    target: OmnibarMatch::Edge {
                        from: from_key,
                        to: to_key,
                    },
                });
            }
        }
    }

    for name in graph_app.list_named_graph_snapshot_names() {
        if let Some(snapshot) = graph_app.peek_named_graph_snapshot(&name) {
            for (_, node) in snapshot.nodes() {
                if node_urls_seen.insert(node.url.clone()) {
                    all_graph_node_candidates.push(OmnibarSearchCandidate {
                        text: format!("{} {}", node.title, node.url),
                        target: OmnibarMatch::NodeUrl(node.url.clone()),
                    });
                }
            }
            for edge in snapshot.edges() {
                let Some(from_node) = snapshot.get_node(edge.from) else {
                    continue;
                };
                let Some(to_node) = snapshot.get_node(edge.to) else {
                    continue;
                };
                let current_from = graph_app
                    .domain_graph()
                    .get_node_by_url(&from_node.url)
                    .map(|(k, _)| k);
                let current_to = graph_app
                    .domain_graph()
                    .get_node_by_url(&to_node.url)
                    .map(|(k, _)| k);
                if let (Some(from_key), Some(to_key)) = (current_from, current_to)
                    && mapped_edge_keys_seen.insert((from_key, to_key))
                {
                    all_graph_edge_candidates.push(OmnibarSearchCandidate {
                        text: format!(
                            "{} {} {} {} {}",
                            edge_type_label(edge.edge_type),
                            from_node.title,
                            from_node.url,
                            to_node.title,
                            to_node.url
                        ),
                        target: OmnibarMatch::Edge {
                            from: from_key,
                            to: to_key,
                        },
                    });
                }
            }
        }
    }

    let local_tab_candidates = tab_candidates_for_keys(graph_app, &local_tab_nodes);
    let all_tab_keys: HashSet<NodeKey> = local_tab_nodes
        .iter()
        .copied()
        .chain(saved_tab_nodes.iter().copied())
        .collect();
    let all_tab_candidates = tab_candidates_for_keys(graph_app, &all_tab_keys);

    match mode {
        OmnibarSearchMode::NodesLocal => ranked_matches(local_node_candidates, query),
        OmnibarSearchMode::NodesAll => ranked_matches(all_graph_node_candidates, query),
        OmnibarSearchMode::TabsLocal => ranked_matches(local_tab_candidates, query),
        OmnibarSearchMode::TabsAll => ranked_matches(all_tab_candidates, query),
        OmnibarSearchMode::EdgesLocal => ranked_matches(local_edge_candidates, query),
        OmnibarSearchMode::EdgesAll => ranked_matches(all_graph_edge_candidates, query),
        OmnibarSearchMode::Mixed => {
            let node_matches = fuzzy_match_node_keys(graph_app.domain_graph(), query);
            if node_matches.is_empty() {
                return ranked_matches(all_graph_node_candidates, query);
            }
            let hop_distances = graph_app
                .focused_selection()
                .primary()
                .map(|context| graph_app.cached_hop_distances_for_context(context))
                .unwrap_or_default();
            let local_tab_set = tab_node_keys_in_tree(tiles_tree);
            if !has_node_panes {
                let node_rank: HashMap<NodeKey, usize> = node_matches
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(idx, key)| (key, idx))
                    .collect();
                let mut ordered_nodes = node_matches;
                ordered_nodes.sort_by_key(|key| {
                    (
                        hop_distances.get(key).copied().unwrap_or(usize::MAX),
                        node_rank.get(key).copied().unwrap_or(usize::MAX),
                    )
                });
                let mut out: Vec<OmnibarMatch> =
                    ordered_nodes.into_iter().map(OmnibarMatch::Node).collect();
                out.extend(ranked_matches(all_graph_node_candidates, query));
                return dedupe_matches_in_order(out);
            }
            let all_tab_ranked_matches =
                ranked_matches(tab_candidates_for_keys(graph_app, &all_tab_keys), query);
            let tab_rank: HashMap<NodeKey, usize> = all_tab_ranked_matches
                .iter()
                .enumerate()
                .filter_map(|(idx, m)| match m {
                    OmnibarMatch::Node(key) => Some((*key, idx)),
                    _ => None,
                })
                .collect();
            let mut local_connected_tabs = Vec::new();
            let mut local_tabs = Vec::new();
            let mut other_frame_connected_tabs = Vec::new();
            let mut other_frame_tabs = Vec::new();
            for candidate in all_tab_ranked_matches {
                let OmnibarMatch::Node(key) = candidate else {
                    continue;
                };
                let connected = hop_distances.contains_key(&key);
                if connected && local_tab_set.contains(&key) {
                    local_connected_tabs.push(key);
                } else if local_tab_set.contains(&key) {
                    local_tabs.push(key);
                } else if connected {
                    other_frame_connected_tabs.push(key);
                } else {
                    other_frame_tabs.push(key);
                }
            }
            local_connected_tabs.sort_by_key(|key| {
                (
                    hop_distances.get(key).copied().unwrap_or(usize::MAX),
                    tab_rank.get(key).copied().unwrap_or(usize::MAX),
                )
            });
            other_frame_connected_tabs.sort_by_key(|key| {
                (
                    hop_distances.get(key).copied().unwrap_or(usize::MAX),
                    tab_rank.get(key).copied().unwrap_or(usize::MAX),
                )
            });
            let mut out: Vec<OmnibarMatch> = local_connected_tabs
                .into_iter()
                .chain(local_tabs)
                .chain(other_frame_connected_tabs)
                .chain(other_frame_tabs)
                .map(OmnibarMatch::Node)
                .collect();
            let mut remaining_nodes = ranked_matches(all_graph_node_candidates, query);
            remaining_nodes.retain(|m| {
                matches!(m, OmnibarMatch::NodeUrl(_))
                    || matches!(m, OmnibarMatch::Node(key) if !all_tab_keys.contains(key))
            });
            remaining_nodes.sort_by_key(|m| match m {
                OmnibarMatch::Node(key) => hop_distances.get(key).copied().unwrap_or(usize::MAX),
                OmnibarMatch::NodeUrl(_) => usize::MAX,
                OmnibarMatch::SearchQuery { .. } => usize::MAX,
                OmnibarMatch::Edge { .. } => usize::MAX,
            });
            out.extend(remaining_nodes);
            dedupe_matches_in_order(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GraphBrowserApp, GraphViewId};
    use crate::graph::{EdgeType, ImportRecord, ImportRecordMembership};
    use crate::shell::desktop::workbench::pane_model::GraphPaneRef;
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use egui_tiles::Tree;
    use euclid::default::Point2D;
    use tempfile::TempDir;

    #[test]
    fn test_provider_suggest_url_duckduckgo() {
        let url = provider_suggest_url(SearchProviderKind::DuckDuckGo, "rust graph");
        assert!(
            url.starts_with("https://duckduckgo.com/ac/?q=rust+graph"),
            "unexpected duckduckgo suggest url: {url}"
        );
    }

    #[test]
    fn test_parse_provider_search_query_modes() {
        assert_eq!(
            parse_provider_search_query("g rust"),
            Some((SearchProviderKind::Google, "rust"))
        );
        assert_eq!(
            parse_provider_search_query("b rust"),
            Some((SearchProviderKind::Bing, "rust"))
        );
        assert_eq!(
            parse_provider_search_query("d rust"),
            Some((SearchProviderKind::DuckDuckGo, "rust"))
        );
        assert!(parse_provider_search_query("n rust").is_none());
    }

    #[test]
    fn test_non_at_matches_for_settings_contextual_order_uses_primary_matches_first() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.chrome_ui.omnibar_preferred_scope = OmnibarPreferredScope::LocalTabs;
        app.workspace.chrome_ui.omnibar_non_at_order =
            OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal;

        let tab_key = app.add_node_and_sync("https://alpha-tab.example".into(), Point2D::zero());
        let mut tiles = egui_tiles::Tiles::default();
        let tab_tile = tiles.insert_pane(TileKind::Node(tab_key.into()));
        let tabs = tiles.insert_tab_tile(vec![tab_tile]);
        let tree = Tree::new("settings_order_contextual", tabs, tiles);

        let (matches, should_load_provider) =
            non_at_matches_for_settings(&mut app, &tree, "alpha", true);

        assert!(!should_load_provider);
        assert_eq!(matches.first().cloned(), Some(OmnibarMatch::Node(tab_key)));
    }

    #[test]
    fn test_non_at_matches_for_settings_provider_first_defers_to_provider_loading() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.chrome_ui.omnibar_preferred_scope = OmnibarPreferredScope::LocalTabs;
        app.workspace.chrome_ui.omnibar_non_at_order =
            OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal;

        let tab_key = app.add_node_and_sync("https://alpha-tab.example".into(), Point2D::zero());
        let mut tiles = egui_tiles::Tiles::default();
        let tab_tile = tiles.insert_pane(TileKind::Node(tab_key.into()));
        let tabs = tiles.insert_tab_tile(vec![tab_tile]);
        let tree = Tree::new("settings_order_provider", tabs, tiles);

        let (matches, should_load_provider) =
            non_at_matches_for_settings(&mut app, &tree, "alpha", true);

        assert!(matches.is_empty());
        assert!(should_load_provider);
    }

    #[test]
    fn test_parse_provider_suggestion_body_ddg_shape() {
        let body = r#"[{"phrase":"rust book"},{"phrase":"rust language"}]"#;
        let suggestions = parse_provider_suggestion_body(body, "rust").expect("parse suggestions");
        assert_eq!(suggestions.first().map(String::as_str), Some("rust"));
        assert!(suggestions.iter().any(|s| s == "rust book"));
        assert!(suggestions.iter().any(|s| s == "rust language"));
    }

    #[test]
    fn test_parse_provider_suggestion_body_osjson_shape() {
        let body = r#"["rust",["rust book","rust language"],[],[]]"#;
        let suggestions = parse_provider_suggestion_body(body, "rust").expect("parse suggestions");
        assert_eq!(suggestions.first().map(String::as_str), Some("rust"));
        assert!(suggestions.iter().any(|s| s == "rust book"));
        assert!(suggestions.iter().any(|s| s == "rust language"));
    }

    #[test]
    fn test_provider_parsed_metadata_cache_key_is_namespaced() {
        assert_eq!(
            provider_parsed_metadata_cache_key(SearchProviderKind::Google, "rust"),
            "provider:parsed_suggestions:google:rust"
        );
    }

    #[test]
    fn test_omnibar_match_label_includes_import_record_metadata() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync("https://imported.example".into(), Point2D::zero());
        let node_id = app
            .domain_graph()
            .get_node(key)
            .expect("node")
            .id
            .to_string();
        assert!(app.set_import_records_for_tests(vec![ImportRecord {
            record_id: "import-record:firefox-bookmarks-2026-03-17".to_string(),
            source_id: "import:firefox-bookmarks".to_string(),
            source_label: "Firefox bookmarks".to_string(),
            imported_at_secs: 1_763_500_800,
            memberships: vec![ImportRecordMembership {
                node_id,
                suppressed: false,
            }],
        }]));

        let label = omnibar_match_label(&app, &OmnibarMatch::Node(key));
        assert!(label.contains("Imported: Firefox bookmarks"));
        assert!(label.contains("import-record:firefox-bookmarks-2026-03-17"));
    }

    #[test]
    fn test_parse_provider_suggestion_value_dedupes_and_keeps_fallback_first() {
        let value = serde_json::json!(["rust", ["rust", "rust book", "rust book"], [], []]);
        let suggestions = parse_provider_suggestion_value(&value, "rust")
            .expect("parsed suggestion value should produce output");
        assert_eq!(suggestions.first().map(String::as_str), Some("rust"));
        assert_eq!(
            suggestions
                .iter()
                .filter(|entry| entry.as_str() == "rust book")
                .count(),
            1
        );
    }

    #[test]
    fn test_non_at_contextual_matches_prioritize_local_then_connected_capped() {
        let mut app = GraphBrowserApp::new_for_testing();
        let context = app.add_node_and_sync("https://context.example".into(), Point2D::zero());
        let local_tab = app.add_node_and_sync(
            "https://alpha-local.example".into(),
            Point2D::new(10.0, 0.0),
        );

        let mut connected_nodes = Vec::new();
        for idx in 0..12 {
            let key = app.add_node_and_sync(
                format!("https://alpha-connected-{idx}.example"),
                Point2D::new(20.0 + idx as f32, 0.0),
            );
            let _ = app.add_edge_and_sync(context, key, EdgeType::Hyperlink, None);
            connected_nodes.push(key);
        }
        app.apply_reducer_intents([GraphIntent::SelectNode {
            key: context,
            multi_select: false,
        }]);

        let mut tiles = egui_tiles::Tiles::default();
        let local_leaf = tiles.insert_pane(TileKind::Node(local_tab.into()));
        let root = tiles.insert_tab_tile(vec![local_leaf]);
        let tree = Tree::new("non_at_contextual", root, tiles);

        let matches = non_at_contextual_matches(&mut app, &tree, "alpha", true);
        assert!(!matches.is_empty());
        assert_eq!(matches[0], OmnibarMatch::Node(local_tab));
        let connected_count = matches
            .iter()
            .filter(|m| matches!(m, OmnibarMatch::Node(key) if connected_nodes.contains(key)))
            .count();
        assert!(
            connected_count <= OMNIBAR_CONNECTED_NON_AT_CAP,
            "connected matches should be capped"
        );
    }

    #[test]
    fn test_parse_omnibar_search_query_modes() {
        assert_eq!(
            parse_omnibar_search_query("t rust"),
            (OmnibarSearchMode::TabsLocal, "rust")
        );
        assert_eq!(
            parse_omnibar_search_query("n rust"),
            (OmnibarSearchMode::NodesLocal, "rust")
        );
        assert_eq!(
            parse_omnibar_search_query("N rust"),
            (OmnibarSearchMode::NodesAll, "rust")
        );
        assert_eq!(
            parse_omnibar_search_query("T rust"),
            (OmnibarSearchMode::TabsAll, "rust")
        );
        assert_eq!(
            parse_omnibar_search_query("e rust"),
            (OmnibarSearchMode::EdgesLocal, "rust")
        );
        assert_eq!(
            parse_omnibar_search_query("E rust"),
            (OmnibarSearchMode::EdgesAll, "rust")
        );
        assert_eq!(
            parse_omnibar_search_query("rust"),
            (OmnibarSearchMode::Mixed, "rust")
        );
    }

    #[test]
    fn test_omnibar_tabs_mode_limits_results_to_tab_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let tab_key = app.add_node_and_sync("https://alpha-tab.example".into(), Point2D::zero());
        let non_tab_key =
            app.add_node_and_sync("https://alpha-node.example".into(), Point2D::new(20.0, 0.0));

        let mut tiles = egui_tiles::Tiles::default();
        let tab_tile = tiles.insert_pane(TileKind::Node(tab_key.into()));
        let tabs = tiles.insert_tab_tile(vec![tab_tile]);
        let tree = Tree::new("tabs_mode_test", tabs, tiles);

        let matches =
            omnibar_matches_for_query(&mut app, &tree, OmnibarSearchMode::TabsLocal, "alpha", true);
        assert_eq!(matches, vec![OmnibarMatch::Node(tab_key)]);
        assert!(!matches.contains(&OmnibarMatch::Node(non_tab_key)));
    }

    #[test]
    fn test_omnibar_mixed_mode_prioritizes_tab_nodes_in_detail_mode() {
        let mut app = GraphBrowserApp::new_for_testing();
        let tab_key = app.add_node_and_sync("https://beta-tab.example".into(), Point2D::zero());
        let node_key =
            app.add_node_and_sync("https://beta-node.example".into(), Point2D::new(20.0, 0.0));

        let mut tiles = egui_tiles::Tiles::default();
        let tab_tile = tiles.insert_pane(TileKind::Node(tab_key.into()));
        let tabs = tiles.insert_tab_tile(vec![tab_tile]);
        let tree = Tree::new("mixed_mode_test", tabs, tiles);

        let matches =
            omnibar_matches_for_query(&mut app, &tree, OmnibarSearchMode::Mixed, "beta", true);
        assert!(!matches.is_empty());
        assert_eq!(matches.first().cloned(), Some(OmnibarMatch::Node(tab_key)));
        assert!(matches.contains(&OmnibarMatch::Node(node_key)));
    }

    #[test]
    fn test_omnibar_mixed_mode_prioritizes_related_tabs_for_selected_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let context_key = app.add_node_and_sync("https://context.example".into(), Point2D::zero());
        let related_tab = app.add_node_and_sync(
            "https://alpha-related.example".into(),
            Point2D::new(20.0, 0.0),
        );
        let unrelated_tab = app.add_node_and_sync(
            "https://alpha-unrelated.example".into(),
            Point2D::new(40.0, 0.0),
        );
        app.add_edge_and_sync(context_key, related_tab, EdgeType::Hyperlink, None)
            .expect("edge should be valid");
        app.apply_reducer_intents([GraphIntent::SelectNode {
            key: context_key,
            multi_select: false,
        }]);

        let mut tiles = egui_tiles::Tiles::default();
        let context_tile = tiles.insert_pane(TileKind::Node(context_key.into()));
        let unrelated_tile = tiles.insert_pane(TileKind::Node(unrelated_tab.into()));
        let related_tile = tiles.insert_pane(TileKind::Node(related_tab.into()));
        let tabs = tiles.insert_tab_tile(vec![context_tile, unrelated_tile, related_tile]);
        let tree = Tree::new("mixed_related_test", tabs, tiles);

        let matches =
            omnibar_matches_for_query(&mut app, &tree, OmnibarSearchMode::Mixed, "alpha", true);
        assert!(matches.len() >= 2);
        assert_eq!(matches[0], OmnibarMatch::Node(related_tab));
        assert_eq!(matches[1], OmnibarMatch::Node(unrelated_tab));
    }

    #[test]
    fn test_omnibar_mixed_mode_orders_connected_tabs_by_hop_distance() {
        let mut app = GraphBrowserApp::new_for_testing();
        let context_key = app.add_node_and_sync("https://context.example".into(), Point2D::zero());
        let hop1 =
            app.add_node_and_sync("https://alpha-hop1.example".into(), Point2D::new(10.0, 0.0));
        let hop2 =
            app.add_node_and_sync("https://alpha-hop2.example".into(), Point2D::new(20.0, 0.0));
        let hop3 =
            app.add_node_and_sync("https://alpha-hop3.example".into(), Point2D::new(30.0, 0.0));
        let _ = app.add_edge_and_sync(context_key, hop1, EdgeType::Hyperlink, None);
        let _ = app.add_edge_and_sync(hop1, hop2, EdgeType::Hyperlink, None);
        let _ = app.add_edge_and_sync(hop2, hop3, EdgeType::Hyperlink, None);
        app.apply_reducer_intents([GraphIntent::SelectNode {
            key: context_key,
            multi_select: false,
        }]);

        let mut tiles = egui_tiles::Tiles::default();
        let context_leaf = tiles.insert_pane(TileKind::Node(context_key.into()));
        let hop3_leaf = tiles.insert_pane(TileKind::Node(hop3.into()));
        let hop2_leaf = tiles.insert_pane(TileKind::Node(hop2.into()));
        let hop1_leaf = tiles.insert_pane(TileKind::Node(hop1.into()));
        let root = tiles.insert_tab_tile(vec![context_leaf, hop3_leaf, hop2_leaf, hop1_leaf]);
        let tree = Tree::new("hop_order_test", root, tiles);

        let matches =
            omnibar_matches_for_query(&mut app, &tree, OmnibarSearchMode::Mixed, "alpha-hop", true);
        assert!(matches.len() >= 3);
        assert_eq!(matches[0], OmnibarMatch::Node(hop1));
        assert_eq!(matches[1], OmnibarMatch::Node(hop2));
        assert_eq!(matches[2], OmnibarMatch::Node(hop3));
    }

    #[test]
    fn test_omnibar_mixed_graph_mode_orders_connected_nodes_by_hop_distance() {
        let mut app = GraphBrowserApp::new_for_testing();
        let context_key = app.add_node_and_sync("https://context.example".into(), Point2D::zero());
        let hop1 = app.add_node_and_sync(
            "https://alpha-graph-hop1.example".into(),
            Point2D::new(10.0, 0.0),
        );
        let hop2 = app.add_node_and_sync(
            "https://alpha-graph-hop2.example".into(),
            Point2D::new(20.0, 0.0),
        );
        let _ = app.add_edge_and_sync(context_key, hop1, EdgeType::Hyperlink, None);
        let _ = app.add_edge_and_sync(hop1, hop2, EdgeType::Hyperlink, None);
        app.apply_reducer_intents([GraphIntent::SelectNode {
            key: context_key,
            multi_select: false,
        }]);

        let mut tiles = egui_tiles::Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let tree = Tree::new("graph_hop_order_test", root, tiles);

        let matches = omnibar_matches_for_query(
            &mut app,
            &tree,
            OmnibarSearchMode::Mixed,
            "alpha-graph-hop",
            false,
        );
        assert!(matches.len() >= 2);
        assert_eq!(matches[0], OmnibarMatch::Node(hop1));
        assert_eq!(matches[1], OmnibarMatch::Node(hop2));
    }

    #[test]
    fn test_omnibar_nodes_all_includes_saved_graph_nodes() {
        let temp = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp.path().to_path_buf());
        let _saved_key =
            app.add_node_and_sync("https://saved-node.example".into(), Point2D::zero());
        app.save_named_graph_snapshot("saved-graph")
            .expect("save named graph snapshot");

        app.clear_graph();
        let _active_key = app.add_node_and_sync(
            "https://active-node.example".into(),
            Point2D::new(10.0, 10.0),
        );

        let mut tiles = egui_tiles::Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let tree = Tree::new("nodes_all_test", root, tiles);

        let matches = omnibar_matches_for_query(
            &mut app,
            &tree,
            OmnibarSearchMode::NodesAll,
            "saved-node",
            false,
        );
        assert!(
            matches.contains(&OmnibarMatch::NodeUrl("https://saved-node.example".into())),
            "expected @N results to include saved graph node by URL"
        );
    }

    #[test]
    fn test_omnibar_tabs_all_includes_saved_frame_tabs() {
        let temp = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp.path().to_path_buf());
        let tab_key = app.add_node_and_sync("https://saved-tab.example".into(), Point2D::zero());

        let mut frame_tiles = egui_tiles::Tiles::default();
        let tab_leaf = frame_tiles.insert_pane(TileKind::Node(tab_key.into()));
        let tabs_root = frame_tiles.insert_tab_tile(vec![tab_leaf]);
        let frame_tree = Tree::new("saved_frame", tabs_root, frame_tiles);
        persistence_ops::save_named_frame_bundle(&mut app, "frame:saved-tabs", &frame_tree)
            .expect("save frame bundle");

        let mut current_tiles = egui_tiles::Tiles::default();
        let current_root =
            current_tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let current_tree = Tree::new("current_tree", current_root, current_tiles);

        let matches = omnibar_matches_for_query(
            &mut app,
            &current_tree,
            OmnibarSearchMode::TabsAll,
            "saved-tab",
            true,
        );
        assert_eq!(matches, vec![OmnibarMatch::Node(tab_key)]);
    }

    #[test]
    fn test_omnibar_mixed_mode_includes_other_frame_tabs_after_local_tabs() {
        let temp = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp.path().to_path_buf());
        let local_tab =
            app.add_node_and_sync("https://alpha-local.example".into(), Point2D::zero());
        let saved_tab = app.add_node_and_sync(
            "https://alpha-saved.example".into(),
            Point2D::new(20.0, 0.0),
        );

        let mut current_tiles = egui_tiles::Tiles::default();
        let local_leaf = current_tiles.insert_pane(TileKind::Node(local_tab.into()));
        let current_root = current_tiles.insert_tab_tile(vec![local_leaf]);
        let current_tree = Tree::new("current_tree", current_root, current_tiles);

        let mut frame_tiles = egui_tiles::Tiles::default();
        let saved_leaf = frame_tiles.insert_pane(TileKind::Node(saved_tab.into()));
        let saved_root = frame_tiles.insert_tab_tile(vec![saved_leaf]);
        let frame_tree = Tree::new("saved_frame", saved_root, frame_tiles);
        persistence_ops::save_named_frame_bundle(&mut app, "frame:saved-alpha", &frame_tree)
            .expect("save frame bundle");

        let matches = omnibar_matches_for_query(
            &mut app,
            &current_tree,
            OmnibarSearchMode::Mixed,
            "alpha",
            true,
        );
        assert!(matches.len() >= 2);
        assert_eq!(matches[0], OmnibarMatch::Node(local_tab));
        assert!(matches.contains(&OmnibarMatch::Node(saved_tab)));
    }

    #[test]
    fn test_omnibar_edges_all_includes_saved_graph_edges_when_nodes_map_by_url() {
        let temp = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp.path().to_path_buf());
        let from = app.add_node_and_sync("https://edge-a.example".into(), Point2D::zero());
        let to = app.add_node_and_sync("https://edge-b.example".into(), Point2D::new(20.0, 0.0));
        let _ = app.add_edge_and_sync(from, to, EdgeType::UserGrouped, None);
        app.save_named_graph_snapshot("saved-edge-graph")
            .expect("save named graph snapshot");
        let _ = app.remove_edges_and_log(from, to, EdgeType::UserGrouped);

        let mut tiles = egui_tiles::Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let tree = Tree::new("edges_all_test", root, tiles);

        let matches = omnibar_matches_for_query(
            &mut app,
            &tree,
            OmnibarSearchMode::EdgesAll,
            "edge-a",
            false,
        );
        assert_eq!(matches, vec![OmnibarMatch::Edge { from, to }]);
    }

    #[test]
    fn test_apply_omnibar_edge_match_sets_highlight_and_pair_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app.add_node_and_sync("https://from.example".into(), Point2D::zero());
        let to = app.add_node_and_sync("https://to.example".into(), Point2D::new(20.0, 0.0));
        let mut intents = Vec::new();
        let mut open_mode = None;

        apply_omnibar_match(
            &app,
            OmnibarMatch::Edge { from, to },
            false,
            false,
            &mut intents,
            &mut open_mode,
        );
        app.apply_reducer_intents(intents);

        assert_eq!(
            app.workspace.graph_runtime.highlighted_graph_edge,
            Some((from, to))
        );
        assert!(app.focused_selection().contains(&from));
        assert!(app.focused_selection().contains(&to));
    }

    #[test]
    fn test_apply_omnibar_node_match_opens_in_current_frame_in_detail_mode() {
        let app = GraphBrowserApp::new_for_testing();
        let key = NodeKey::new(7);
        let mut intents = Vec::new();
        let mut open_mode = None;

        apply_omnibar_match(
            &app,
            OmnibarMatch::Node(key),
            true,
            false,
            &mut intents,
            &mut open_mode,
        );

        assert!(intents.iter().any(|intent| {
            matches!(
                intent,
                GraphIntent::SelectNode {
                    key: selected_key,
                    multi_select: false
                } if *selected_key == key
            )
        }));
        assert!(
            !intents
                .iter()
                .any(|intent| { matches!(intent, GraphIntent::OpenNodeFrameRouted { .. }) })
        );
        assert!(matches!(open_mode, Some(ToolbarOpenMode::Tab)));
    }

    #[test]
    fn test_apply_omnibar_node_match_shift_forces_frame_routing() {
        let app = GraphBrowserApp::new_for_testing();
        let key = NodeKey::new(9);
        let mut intents = Vec::new();
        let mut open_mode = None;

        apply_omnibar_match(
            &app,
            OmnibarMatch::Node(key),
            true,
            true,
            &mut intents,
            &mut open_mode,
        );

        assert!(intents.iter().any(|intent| {
            matches!(
                intent,
                GraphIntent::OpenNodeFrameRouted {
                    key: routed_key,
                    prefer_frame: None
                } if *routed_key == key
            )
        }));
        assert!(
            !intents
                .iter()
                .any(|intent| { matches!(intent, GraphIntent::SelectNode { .. }) })
        );
        assert!(open_mode.is_none());
    }

    #[test]
    fn test_apply_omnibar_node_url_existing_routes_frame_open_in_detail_mode() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync("https://node-url.example".into(), Point2D::zero());
        let mut intents = Vec::new();
        let mut open_mode = None;

        apply_omnibar_match(
            &app,
            OmnibarMatch::NodeUrl("https://node-url.example".into()),
            true,
            false,
            &mut intents,
            &mut open_mode,
        );

        assert!(intents.iter().any(|intent| {
            matches!(
                intent,
                GraphIntent::OpenNodeFrameRouted {
                    key: routed_key,
                    prefer_frame: None
                } if *routed_key == key
            )
        }));
        assert!(open_mode.is_none());
    }

    #[test]
    fn test_apply_omnibar_node_url_new_keeps_open_selected_mode_for_new_node() {
        let app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();
        let mut open_mode = None;

        apply_omnibar_match(
            &app,
            OmnibarMatch::NodeUrl("https://new-node-url.example".into()),
            true,
            false,
            &mut intents,
            &mut open_mode,
        );

        assert!(intents.iter().any(|intent| {
            matches!(
                intent,
                GraphIntent::CreateNodeAtUrlAndOpen { url, .. }
                    if url == "https://new-node-url.example"
            )
        }));
        assert!(open_mode.is_none());
    }
}
