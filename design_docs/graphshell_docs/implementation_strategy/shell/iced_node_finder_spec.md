<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Node Finder Spec

**Date**: 2026-04-29
**Status**: Canonical / Active — added 2026-04-29 by the omnibar-split simplification
**Scope**: A Modal surface for fuzzy-searching graph nodes by title, tag,
address, or content snapshot. Activation opens-or-activates the selected
node in a destination Pane. Modeled after Zed/VSCode's Cmd+P file finder
and Sublime's "Goto Anything"; sibling to the
[Command Palette](iced_command_palette_spec.md) (`Ctrl+Shift+P`,
action-shaped) and [Omnibar](iced_omnibar_spec.md) (`Ctrl+L`, URL-shaped).

**Code-sample mode**: **Illustrative signatures**. Concrete S3/S4 code lives
in the implementation, not this spec.

**Related**:

- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — Application skeleton, slot model
- [`iced_omnibar_spec.md`](iced_omnibar_spec.md) — sibling URL-entry surface; routes non-URL queries here
- [`iced_command_palette_spec.md`](iced_command_palette_spec.md) — sibling action-dispatch surface
- [`iced_browser_amenities_spec.md` §4](iced_browser_amenities_spec.md) — find-in-graph row redirected here
- [`../aspect_command/command_surface_interaction_spec.md` §4.5A](../aspect_command/command_surface_interaction_spec.md) — canonical Node Finder spec (added 2026-04-29 same-day)
- [`2026-04-28_iced_jump_ship_plan.md` §4.10](2026-04-28_iced_jump_ship_plan.md) — coherence guarantee for the Node Finder
- [`../subsystem_history/SUBSYSTEM_HISTORY.md`](../subsystem_history/SUBSYSTEM_HISTORY.md) — recency aggregate (default empty-query ranking)

---

## 1. Intent

The Node Finder answers "I want to open a node and I have a vague idea
of what it is" — by title, by a tag, by part of the URL, by content
keyword. It is a `Ctrl+P`-shaped surface, sibling to the omnibar
(URL-shaped) and the Command Palette (action-shaped).

What the Node Finder **does**:

- accepts free-text input;
- shows fuzzy-ranked nodes matching the input across (title, tag,
  address, content snapshot);
- empty input shows recently-active nodes (recency-ranked);
- activation opens-or-activates the selected node in a destination Pane.

What the Node Finder **does not** do:

- ❌ open a typed URL — that's the omnibar.
- ❌ invoke commands — that's the Command Palette.
- ❌ create a new node — node creation goes through `GraphIntent::CreateNode`
  via the Command Palette or other dedicated actions.
- ❌ fuzzy-match across actions or chrome surfaces — only graph nodes.

The split is deliberate per the 2026-04-29 simplification: each
keyboard-shortcut surface owns one verb.

---

## 2. Invocation

`Ctrl+P` (canonical, Zed/VSCode-shaped) opens the Node Finder. Other
trigger sources:

- **Tree Spine row "Search nodes…" entry** (planned) opens the finder.
- **Omnibar `RouteToNodeFinder`**: when the user submits non-URL-shaped
  text in the omnibar, the omnibar dispatches
  `Message::NodeFinderOpenWithQuery(text)` to pre-fill the finder.
- **Programmatic** (rare): an action's effect that wants to ask the
  user to pick a node.

All paths converge on `Message::NodeFinderOpen { initial_query, origin }`.
Origin is recorded for provenance per
[`subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md).

---

## 3. Widget Tree

```rust
fn node_finder_overlay(state: &State) -> Option<Element<'_, Message, GraphshellTheme>> {
    state.node_finder.is_open.then(|| {
        modal(
            container(
                column![
                    text_input(
                        &state.node_finder.query,
                        Message::NodeFinderQuery,
                    )
                    .on_submit(Message::NodeFinderSubmitFocused)
                    .id(text_input::Id::new("node_finder_input"))
                    .placeholder("Search nodes by title, tag, URL, or content…"),

                    horizontal_rule(),

                    node_finder_results_list(
                        &state.node_finder.results,
                        state.node_finder.focused_index,
                    ),

                    node_finder_footer(state),  // hint text, "Open as URL…", web-search fallback
                ]
                .spacing(8)
                .padding(12)
            )
            .style(node_finder_container_style)
            .max_width(640)
            .max_height(480)
        )
        .on_blur(Message::NodeFinderCloseAndRestoreFocus)
        .into()
    })
}
```

The Modal layout is intentionally similar to the Command Palette
([iced_command_palette_spec.md §2.2](iced_command_palette_spec.md))
so the two surfaces feel consistent. The differences are in the result
row shape (§4) and the dispatched intent kind (§7).

---

## 4. Result Row Shape

```rust
pub struct NodeFinderResult {
    pub node_key: NodeKey,
    pub title: String,                       // node title or address fallback
    pub address: String,                     // canonical address
    pub node_type: NodeTypeBadge,            // Web | File | Tool | Internal | Clip
    pub match_source: MatchSource,           // Title | Tag | Url | Content
    pub match_snippet: Option<String>,       // for Content matches: highlighted excerpt
    pub recency_rank: Option<u32>,           // for empty-query ranking
}

pub enum MatchSource {
    Title,
    Tag,
    Url,
    Content,
    Recency,                                  // empty-query results
}
```

Each row renders:

- **Title** prominently (verb-target wording does not apply — this is
  node display, not action display).
- **Address** as secondary text, smaller / dimmer.
- **Node type badge** (small chip: Web, File, Tool, Internal, Clip).
- **Match source badge** (small chip: Title / Tag / URL / Content)
  showing where the query matched.
- **Content match snippet** if `match_source == Content`, with the
  matched substring highlighted.

Empty-query rows (recency-ranked) hide the match source badge and may
show a `Recently active` chip instead.

---

## 5. State Shape

```rust
pub struct NodeFinderState {
    pub is_open: bool,
    pub origin: NodeFinderOrigin,
    pub query: String,
    pub results: Vec<NodeFinderResult>,
    pub focused_index: Option<usize>,
    pub focus_token: Option<widget::Id>,     // saved iced focus id at open time
    pub current_request: Option<RankRequestId>,
}

pub enum NodeFinderOrigin {
    KeyboardShortcut,                        // Ctrl+P
    OmnibarRoute(String),                    // routed from omnibar with non-URL query
    TreeSpineRow,                            // user clicked a "Search nodes…" entry
    ProgrammaticByAction(ActionId),
}
```

Per the omnibar-split simplification, the Node Finder stores its own
`focus_token`; there is no shared `CommandBarFocusTarget`.

---

## 6. Filtering and Ranking

Empty query: `results` shows recently-active nodes ranked by recency
(per `SUBSYSTEM_HISTORY` recency aggregate). The runtime exposes:

```rust
pub trait NodeFinderViewModel {
    fn recently_active(&self, limit: usize) -> Vec<NodeFinderResult>;
    fn rank_for_query(
        &self,
        query: String,
    ) -> impl Future<Output = (RankRequestId, Vec<NodeFinderResult>)>;
}
```

Non-empty query: `rank_for_query` runs fuzzy matching over the runtime's
graph index (title + tag + address + content snapshot). Ranking is
async; results return via `Message::NodeFinderRankResultsReady`.

Cancellation by request-id supersession: each `NodeFinderQuery` schedules
a new `RankRequestId`; stale results are dropped.

The exact ranking algorithm (BM25 / SkimMatch / token-frequency / etc.)
is the runtime's call; the Node Finder UI just renders what comes back.

---

## 7. Message Contract

```rust
pub enum Message {
    NodeFinderOpen { origin: NodeFinderOrigin },
    NodeFinderOpenWithQuery(String),
    NodeFinderClose,
    NodeFinderCloseAndRestoreFocus,

    NodeFinderQuery(String),

    NodeFinderFocusNext,                     // ArrowDown / Tab
    NodeFinderFocusPrev,                     // ArrowUp / Shift+Tab
    NodeFinderFocusedRowChanged(usize),

    NodeFinderSubmitFocused,                 // Enter on focused row
    NodeFinderResultSelected(NodeKey),       // click on a row

    NodeFinderRankResultsReady {
        request_id: RankRequestId,
        results: Vec<NodeFinderResult>,
    },

    // Footer fallbacks
    NodeFinderOpenAsUrl(String),             // "Open as URL…" — pre-fill omnibar
    NodeFinderWebSearch(String),             // "Search the web for…" — config-gated
}
```

---

## 8. Update Routing

```rust
fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
        Message::NodeFinderOpen { origin } => {
            self.node_finder = NodeFinderState::open_for(
                origin,
                self.current_focused_widget_id(),
                self.runtime.node_finder().recently_active(NODE_FINDER_RECENT_LIMIT),
            );
            return widget::focus(text_input::Id::new("node_finder_input"));
        }

        Message::NodeFinderOpenWithQuery(query) => {
            self.node_finder.is_open = true;
            self.node_finder.origin = NodeFinderOrigin::OmnibarRoute(query.clone());
            self.node_finder.query = query.clone();
            self.node_finder.focus_token = self.current_focused_widget_id();
            // Spawn rank for the routed query
            let req = self.runtime.node_finder().next_rank_request_id();
            self.node_finder.current_request = Some(req);
            return Task::batch([
                widget::focus(text_input::Id::new("node_finder_input")),
                Task::perform(
                    self.runtime.node_finder().rank_for_query(query),
                    move |(rid, results)| Message::NodeFinderRankResultsReady {
                        request_id: rid,
                        results,
                    },
                ),
            ]);
        }

        Message::NodeFinderQuery(query) => {
            self.node_finder.query = query.clone();
            if query.is_empty() {
                self.node_finder.results =
                    self.runtime.node_finder().recently_active(NODE_FINDER_RECENT_LIMIT);
                self.node_finder.focused_index = None;
                Task::none()
            } else {
                let req = self.runtime.node_finder().next_rank_request_id();
                self.node_finder.current_request = Some(req);
                Task::perform(
                    self.runtime.node_finder().rank_for_query(query),
                    move |(rid, results)| Message::NodeFinderRankResultsReady {
                        request_id: rid,
                        results,
                    },
                )
            }
        }

        Message::NodeFinderResultSelected(node_key) => {
            // Open or activate the selected node per the user's destination rule
            // (active Pane / new Pane / replace focused — same rule the omnibar uses).
            self.runtime.emit(WorkbenchIntent::OpenNode {
                node_key,
                destination: self.user_settings.node_open_destination_rule,
            });
            return Task::done(Message::NodeFinderCloseAndRestoreFocus);
        }

        Message::NodeFinderCloseAndRestoreFocus => {
            self.node_finder.close();
            return restore_focus(self.node_finder.focus_token.take());
        }

        Message::NodeFinderRankResultsReady { request_id, results } => {
            if Some(request_id) != self.node_finder.current_request {
                return Task::none();
            }
            self.node_finder.results = results;
            self.node_finder.focused_index = (!results.is_empty()).then_some(0);
            Task::none()
        }

        Message::NodeFinderOpenAsUrl(text) => {
            // Footer fallback: route the typed text to the omnibar in Input mode.
            self.node_finder.close();
            self.omnibar.draft = text;
            self.omnibar.mode = OmnibarMode::Input;
            return widget::focus(text_input::Id::new("omnibar"));
        }

        // ... other arms ...
    }
}
```

### 8.1 Activation routing

`WorkbenchIntent::OpenNode` carries a `destination` field:

- `ActivePane` (default): activate the node in the currently focused
  tile Pane, or the most-recent active Pane if focus isn't on a tile.
- `NewPane`: create a new tile Pane in a Split adjacent to the
  current focus.
- `ReplaceFocusedPane`: replace the focused Pane's content with the
  selected node.

The destination rule is a `WorkbenchProfile` setting; the Node Finder
emits the intent with that rule, the runtime + workbench handle
placement. This is the same routing the omnibar uses (per
[iced_omnibar_spec.md §6.2](iced_omnibar_spec.md)).

---

## 9. Footer Fallbacks

Two footer entries handle the "this query isn't a node — what now?"
case:

- **"Open as URL…"** — opens the omnibar in Input mode pre-filled with
  the typed text. Always available.
- **"Search the web for X"** — dispatches to a configured web-search
  engine (per Settings); the result either creates a new node from
  the search-engine URL or opens search results in a tile Pane. Only
  shown when web-search fallback is enabled in Settings.

Footer entries are not result rows; they appear below the rule
separator and don't contribute to the keyboard navigation index unless
results are empty.

---

## 10. IME and Accessibility

`text_input` (iced 0.14+) is IME-aware out of the box.

AccessKit role mapping:

- Modal overlay → `dialog`
- `text_input` → `searchbox`
- Result list → `listbox`
- Result row → `option`
- Footer entries → `button`

Keyboard navigation:

- `Ctrl+P` opens the finder.
- Arrow keys navigate the result list.
- Enter activates the focused row.
- Escape dismisses; restores focus to `focus_token`.
- Tab can cycle: input → result list → footer entries.

WCAG 2.2 AA targets per [DOC_POLICY.md](../../DOC_POLICY.md):

- SC 2.4.3 (Focus Order)
- SC 2.4.11 (Focus Appearance)
- SC 2.5.8 (Target Size, ≥ 32×32 dp for rows and footer)

These targets land at Stage E and gate via `UxProbeSet` AT validation.

---

## 11. Coherence Guarantee

Per [iced jump-ship plan §4.10](2026-04-28_iced_jump_ship_plan.md) (added
in the same 2026-04-29 commit as this spec):

> **Node Finder**: Searching never mutates graph truth. Activation
> emits a single `WorkbenchIntent::OpenNode` per selection; no node is
> created or deleted from the finder itself. Results reflect current
> graph truth via the runtime's index — stale results are dropped on
> request-id supersession.

This spec preserves the guarantee:

- Query / results / focus state lives only in `NodeFinderState`; never
  written to graph state.
- `NodeFinderResultSelected` emits one `WorkbenchIntent::OpenNode`; the
  finder closes after dispatch and waits for confirmation via the
  Activity Log, not in-finder feedback.
- "Open as URL…" routes to the omnibar (which itself emits
  `OpenAddress`); "Search the web for…" emits a single web-search
  intent.
- The Node Finder does not create nodes. If a user wants to create a
  node from non-matching text, they use the omnibar (URL → new node)
  or the Command Palette (`Create node` action).

---

## 12. Open Items

- **Search index implementation**: BM25 vs SkimMatch vs hybrid; runtime-
  side decision, not iced-side.
- **Result preview pane**: a future enhancement could show a preview
  panel beside the result list (Quicklook-style) for the focused
  result. Not in first bring-up.
- **Saved searches**: persist a query + its results as a graphlet for
  later reuse. Tracked in
  [iced_browser_amenities_spec.md §4](iced_browser_amenities_spec.md)
  open items.
- **Visual style / animation**: Modal enter/exit, row hover, match-
  highlight color. Stage F polish.
- **Per-source filter chips**: future enhancement to filter results by
  Title / Tag / URL / Content via inline chips. Not in first bring-up.

---

## 13. Bottom Line

The Node Finder is one Modal overlay (`Ctrl+P`) with a `text_input` and
a flat ranked list of graph nodes — Zed/VSCode-shaped, sibling to the
Command Palette and the Omnibar. It is the canonical surface for
"open a node by what I know about it." Activation emits one
`WorkbenchIntent::OpenNode` per selection; routing of the destination
Pane uses the same `WorkbenchProfile` rule as the omnibar. State is
widget-local; runtime ranking returns through Subscription with
request-id supersession; nothing in the finder mutates graph truth.

Together with the simplified omnibar (URL only) and the Command Palette
(actions only), the three surfaces split the four-role egui-era omnibar
into focused, single-verb keyboard-friendly surfaces.
