<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Search Providers & Fuzzy Match Spec

**Date**: 2026-04-30
**Status**: Canonical / Active
**Scope**: Two related primitives the iced surfaces share — a
**SearchProvider registry** for omnibar URL completion, Node Finder
web-fallback, and any future search integration; and a **shared fuzzy-
match adapter** built on [`nucleo`](https://crates.io/crates/nucleo) (the
Helix crate; pure Rust, fast, used by file finders) so the Node Finder,
Command Palette, and `@`-references in the agent pane all use the same
ranking primitive. Small spec — one `SearchProvider` trait, one fuzzy
adapter.

**Related**:

- [`iced_omnibar_spec.md`](iced_omnibar_spec.md) — URL completion provider consumer
- [`iced_node_finder_spec.md`](iced_node_finder_spec.md) — graph-node fuzzy ranking + web-search fallback consumer
- [`iced_command_palette_spec.md`](iced_command_palette_spec.md) — action fuzzy ranking consumer
- [`iced_agent_pane_spec.md` §6.1](iced_agent_pane_spec.md) — `@`-reference autocomplete consumer
- [`../system/graphshell_net_spec.md` §3.5](../system/graphshell_net_spec.md) — provider HTTP traffic flows through `graphshell-net`
- [`../aspect_control/settings_and_permissions_spine_spec.md`](../aspect_control/settings_and_permissions_spine_spec.md) — provider allowlist + default-search settings live here
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — Registry Architecture (atomic registries)

---

## 1. Intent

Two unrelated-but-shaped-similarly needs across iced surfaces:

1. **Search Providers** — multiple sources of suggestions / search
   results: URL completion (history-by-URL, bookmark-URLs), web search
   engines (DuckDuckGo / Kagi / etc.), graph-search (the runtime's node
   index), Verse community search (future). Each surface that consumes
   one of these wants a uniform interface.

2. **Fuzzy match** — Node Finder ranks graph nodes by query;
   Command Palette ranks actions by query; Agent Pane `@`-references
   autocomplete graph entities. All three want the same fuzzy-scoring
   primitive so behavior and ranking quality are consistent.

This spec defines both. They are bundled because they're typically
consumed together (a SearchProvider returns ranked candidates; the
ranking uses fuzzy match) and because each is small.

---

## 2. SearchProvider Trait

```rust
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    /// Provider category — drives default allowlists, permission
    /// keys, and surface-routing.
    fn category(&self) -> ProviderCategory;

    /// Issue a query against this provider. Async; cancellable through
    /// the request id.
    fn query(
        &self,
        request_id: RequestId,
        query: String,
        scope: ScopePath,
    ) -> impl Future<Output = ProviderResult>;

    /// Optional: feedback on a selected result (for ranking
    /// improvements over time; persona-scoped recency).
    fn feedback(&self, _request_id: RequestId, _selected: ResultIndex) {}
}

pub enum ProviderCategory {
    UrlCompletion,        // history-by-URL, bookmark-URL
    WebSearch,            // DuckDuckGo, Kagi, Google, etc.
    GraphSearch,          // runtime's graph index
    AgentReference,       // for agent pane @-references
    VerseSearch,          // Verse community search (Tier 2)
    Other(String),        // mod-defined categories
}

pub struct ProviderResult {
    pub request_id: RequestId,
    pub items: Vec<ResultItem>,
    pub status: ProviderStatus,            // ok / partial / cancelled / error
}

pub struct ResultItem {
    pub label: String,
    pub detail: Option<String>,
    pub address: Option<Url>,              // for URL/web/Verse providers
    pub node_key: Option<NodeKey>,         // for graph providers
    pub action: Option<ActionId>,          // for action ranking
    pub score: f32,                        // 0.0 - 1.0; provider-specific scoring
    pub source_badge: SourceBadge,         // icon + "from history" / "web" / etc.
}
```

### 2.1 ProviderRegistry (atomic)

Per [TERMINOLOGY.md Registry Architecture](../../TERMINOLOGY.md),
`ProviderRegistry` is an **atomic registry** holding all registered
SearchProviders. Mods register providers via the standard atomic
registry pattern.

```rust
pub trait ProviderRegistry {
    fn register(&mut self, provider: Arc<dyn SearchProvider>);
    fn unregister(&mut self, id: ProviderId);
    fn get(&self, id: ProviderId) -> Option<Arc<dyn SearchProvider>>;
    fn for_category(&self, category: ProviderCategory) -> Vec<Arc<dyn SearchProvider>>;
}
```

A surface that wants to query asks for all providers in its category
and dispatches per the allowlist:

```rust
let providers = registry.for_category(ProviderCategory::UrlCompletion)
    .into_iter()
    .filter(|p| settings.is_provider_allowed(p.id(), scope))
    .collect();

let futures: Vec<_> = providers.iter().map(|p| p.query(req, q.clone(), scope)).collect();
let results = futures::future::join_all(futures).await;
```

Per-provider allowlist follows the
[settings + permissions spine](../aspect_control/settings_and_permissions_spine_spec.md):
`net.providers.<provider_id>` permission key.

### 2.2 Provider HTTP through graphshell-net

When a provider needs outbound HTTP (web search engines, remote
search APIs), the request flows through `graphshell-net`'s
`ProviderRequest` per
[graphshell-net §3.5](../system/graphshell_net_spec.md). Providers
do not roll their own HTTP clients.

### 2.3 Default catalog

Ships with Graphshell at the default scope:

| Provider | Category | Default state |
|---|---|---|
| `history-by-url` | UrlCompletion | enabled |
| `bookmark-urls` | UrlCompletion | enabled |
| `graph-node-finder` | GraphSearch | enabled |
| `action-ranker` | (used internally for Command Palette) | enabled |
| `web-search-default` | WebSearch | configurable; default = none (user-picks) |
| `verse-community` | VerseSearch | disabled by default (Tier 2 mod) |

User selects their default web-search engine in
`verso://settings/persona` (persona-scope setting `default_web_search`).

---

## 3. Fuzzy Match Adapter

`graphshell-fuzzy` wraps [nucleo](https://crates.io/crates/nucleo) and
exposes one trait + one shared instance:

```rust
pub trait FuzzyRanker: Send + Sync {
    fn rank<'a>(
        &self,
        query: &str,
        candidates: impl Iterator<Item = FuzzyCandidate<'a>>,
        config: FuzzyConfig,
    ) -> Vec<RankedCandidate<'a>>;
}

pub struct FuzzyCandidate<'a> {
    pub key: u64,                          // stable identity for the candidate
    pub haystack: &'a str,                 // primary searchable text
    pub additional: &'a [&'a str],         // optional extra fields (tags, address, etc.)
}

pub struct RankedCandidate<'a> {
    pub key: u64,
    pub score: u32,                        // nucleo-native score
    pub matched_indices: Vec<usize>,       // for highlight rendering
    pub primary_match: bool,               // matched in haystack vs additional
}

pub struct FuzzyConfig {
    pub case_sensitive: bool,
    pub smart_case: bool,                  // case-sensitive only if query has uppercase
    pub normalize_unicode: bool,           // accent stripping
    pub max_results: usize,
}
```

### 3.1 Why nucleo

- Pure Rust (no FFI, no `fzy` C wrapper).
- Used by Helix; battle-tested.
- Fast: parallel scoring on large haystacks.
- Returns matched-indices for highlight rendering (which Node Finder
  and Command Palette both want).

Rejected alternatives:

- **skim**: Rust port of fzf; fine but does more than we need
  (interactive UI, finder library).
- **sublime_fuzzy**: simpler but slower and no parallel ranking.
- **Hand-rolled**: re-implementing scoring is unjustified given
  nucleo's quality.

### 3.2 Shared instance

One `Arc<dyn FuzzyRanker>` lives in `graphshell-runtime`; surfaces
read it from the `FrameViewModel`:

```rust
let ranker: &dyn FuzzyRanker = view_model.fuzzy();
let ranked = ranker.rank(query, candidates, config);
```

Sharing the instance means consistent behavior and config across
surfaces. Per-surface tuning happens via `FuzzyConfig` (e.g., Node
Finder uses `smart_case = true`; Command Palette uses
`case_sensitive = false`).

### 3.3 Match-highlight rendering

`RankedCandidate.matched_indices` enables highlighted matched
characters in result rows:

```rust
fn render_with_highlight(text: &str, indices: &[usize]) -> Element<Message> {
    // Render text with matched character ranges in the
    // theme.colors.accent_subtle background.
}
```

Highlight rendering is a small helper in `graphshell-iced-widgets`;
all surfaces use it identically.

### 3.4 Update routing

Fuzzy ranking is **synchronous** for small candidate sets (≤ 1000
items: actions, recent nodes, autocomplete suggestions). Returns
within a frame, no Subscription needed.

For large candidate sets (graph-node fuzzy match across 10k+ nodes),
ranking spawns onto a background `Task` per the
[Node Finder spec §6](iced_node_finder_spec.md); results return via
Subscription with request-id supersession.

---

## 4. Surface Consumption

### 4.1 Omnibar URL completion

Per [iced_omnibar_spec.md §7](iced_omnibar_spec.md):

```text
OmnibarInput(text)
  → for_category(UrlCompletion)
     → history-by-url provider.query(text, [persona, default])
     → bookmark-urls provider.query(text, [persona, default])
  → merge results, fuzzy-rank by URL+title
  → return as Vec<UrlCompletionItem>
```

### 4.2 Node Finder

Per [iced_node_finder_spec.md §6](iced_node_finder_spec.md):

```text
NodeFinderQuery(text)
  → for_category(GraphSearch)
     → graph-node-finder provider.query(text, current scope path)
  → fuzzy-rank locally (titles + tags + addresses + content snippets)
  → return as Vec<NodeFinderResult>

(empty query → recently-active nodes via SUBSYSTEM_HISTORY recency,
 no fuzzy ranking)
```

The "Search the web for X" footer fallback dispatches through
`for_category(WebSearch)` filtered by `default_web_search`.

### 4.3 Command Palette

Per [iced_command_palette_spec.md §2.4](iced_command_palette_spec.md):

```text
PaletteQuery(text)
  → ActionRegistryViewModel.rank_for_query(text, scope)
     → fuzzy-rank actions by (label, description, category) tokens
  → return Vec<RankedAction>
```

The Command Palette's ranker is the action-specific fuzzy adapter,
implemented as a thin wrapper around the shared `FuzzyRanker` with
action-shaped `FuzzyCandidate`.

### 4.4 Agent Pane @-references

Per [iced_agent_pane_spec.md §6.1](iced_agent_pane_spec.md):

```text
AgentInputAtTrigger
  → for_category(AgentReference)
     → graph-node-finder provider (top recent + matched)
     → graphlet provider (matching graphlets)
     → selection provider (current selection summary)
  → fuzzy-rank by query so far
  → render autocomplete dropdown
```

Reuses the same `graph-node-finder` provider as Node Finder (different
caller, same backing index).

---

## 5. Permissions and Settings

Per the
[settings + permissions spine](../aspect_control/settings_and_permissions_spine_spec.md):

| Setting | Scope | Default | Description |
|---|---|---|---|
| `default_web_search` | persona | `none` (user-prompted on first use) | which WebSearch provider answers "Search the web for X" |
| `provider_allowlist` | persona | canonical providers enabled | per-provider allow/deny |
| `provider_allowlist.<id>` | graph | inherits persona | graph-specific override |

Provider HTTP traffic gates on the
[graphshell-net permission keys](../system/graphshell_net_spec.md):
`net.providers.<provider_id>`.

A user denying a provider at any scope removes it from the result
fan-out for queries scoped to or below that scope.

---

## 6. Coherence Guarantees

Per the
[iced jump-ship plan §4.10 omnibar / Node Finder / Command Palette
guarantees](2026-04-28_iced_jump_ship_plan.md):

> Searching never mutates graph truth. Submission emits an explicit
> intent; results reflect current truth.

This spec preserves and tightens:

- Provider queries are read-only (`SearchProvider::query` returns a
  result; no graph mutation as a side effect).
- `feedback()` may update **persona-scope** recency state, but never
  mutates graph truth or ranks candidates differently for one user
  at the expense of another's scoping.
- Fuzzy ranking is **deterministic** for a given query + candidate
  set + config; no randomization, no "tie-broken by recency" without
  explicit recency input.
- Permission denial is **explicit**: a denied provider returns
  `ProviderStatus::PermissionDenied` and surfaces in the Activity
  Log.

---

## 7. Open Items

- **Provider catalog appendix**: this spec defines the trait and the
  default registrations; the canonical list of provider IDs and their
  configs lives in code + a separate appendix.
- **Per-provider rate limits and backoff**: handled by
  `graphshell-net`; provider-specific tuning is a future enhancement.
- **Result merging policy**: when multiple providers return for the
  same category (e.g., two URL-completion providers), how are scores
  normalized across providers? Currently per-provider scores just
  concatenate ranked; a normalization pass is a future enhancement.
- **Cross-provider deduplication**: same URL from history and
  bookmark provider: dedupe by canonical URL? Yes, but the dedupe
  rule needs explicit specification.
- **Mod-provider audit / sandboxing**: a third-party provider mod
  could phone home with user queries. Trust grants via the mod
  permission system (`net.mod.<id>`); per-mod query-redaction is a
  future enhancement.

---

## 8. Bottom Line

One `SearchProvider` trait + atomic registry covers every search
fan-out across iced surfaces (omnibar URL completion, Node Finder
graph search + web fallback, Command Palette action ranking, Agent
pane @-references, future Verse community search). One `FuzzyRanker`
trait wrapping `nucleo` covers every fuzzy-match need with consistent
behavior and matched-indices for highlight rendering. Provider HTTP
flows through `graphshell-net`; allowlists follow the settings spine;
results never mutate graph truth.

Small spec, broad reach: each iced spec that previously had its own
search/ranking sketch now references this one for both pieces.
