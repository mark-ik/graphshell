# Toolbar Omnibar Behavior Spec

**Date:** 2026-03-12  
**Status:** Canonical interaction contract  
**Priority:** Tier 3 / primary UX surface

**Related docs:**

- [`input_interaction_spec.md`](./input_interaction_spec.md)
- [`../system/register/action_registry_contract_spec.md`](../system/register/action_registry_contract_spec.md)
- [`../system/register/input_registry_spec.md`](../system/register/input_registry_spec.md)
- [`../../technical_architecture/ARCHITECTURAL_OVERVIEW.md`](../../technical_architecture/ARCHITECTURAL_OVERVIEW.md)
- [`../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../technical_architecture/2026-03-12_specification_coverage_register.md)

**External standards anchors:**

- WHATWG URL
- HTTP request/submission semantics

---

## 1. Purpose and Scope

This spec defines the behavior contract for the toolbar omnibar / address bar.

It governs:

- query parsing,
- command-vs-search-vs-URL arbitration,
- completion provider ordering,
- local/contextual/global search behavior,
- provider suggestion behavior,
- submission behavior,
- history and persistence interactions.

It does not govern:

- the low-level `ActionRegistry` dispatch contract,
- focus-capture semantics outside omnibar-local interaction,
- webview or browser navigation internals after submission is handed off.

---

## 2. Canonical Role

The omnibar is a unified entry surface for:

- URL/navigation submission,
- contextual node/tab/edge search,
- provider-backed web search suggestions,
- command-surface-adjacent discovery.

Normative rule:

- the omnibar is not only an address field and not only a command field,
- it is a Graphshell arbitration surface across those intents.

---

## 3. Query Parsing Contract

### 3.1 Scope prefixes

Current non-`@` scope prefixes:

- `T` -> tabs all
- `t` / `tab` -> tabs local
- `N` -> nodes all
- `n` / `node` -> nodes local
- `E` -> edges all
- `e` / `edge` -> edges local

If no recognized prefix is present:

- the query is `Mixed`.

Normative rule:

- explicit prefixes must override default contextual scope inference,
- prefix parsing trims leading/trailing whitespace and consumes only the first token as scope marker.

### 3.2 Provider prefixes

Current provider prefixes:

- `g` / `google`
- `b` / `bing`
- `d` / `ddg` / `duckduckgo`

If a provider prefix is recognized:

- the remainder of the query is interpreted as provider-search input.

Normative rule:

- provider prefix interpretation is explicit and case-insensitive,
- provider prefix behavior is independent from local graph/tab scope prefixes.

---

## 4. Search and Completion Model

### 4.1 Match families

The omnibar may surface matches from multiple families:

- local tabs
- global tabs
- local nodes
- global nodes
- local edges
- global edges
- connected nodes from the current context
- provider search suggestions

Normative rule:

- match families are semantically distinct,
- deduplication preserves first-seen order after ordering policy is applied.

### 4.2 Contextual matching

For non-`@` contextual behavior:

- local tabs are treated as a primary contextual source,
- connected nodes may be ranked using hop distance from the current focus/selection context,
- contextual matches may be preferred before global fallbacks depending on user settings.

### 4.3 Settings-driven ordering

Current non-`@` ordering presets:

- `ContextualThenProviderThenGlobal`
- `ProviderThenContextualThenGlobal`

Current preferred-scope options include:

- `Auto`
- `LocalTabs`
- `ConnectedNodes`
- `ProviderDefault`
- `GlobalNodes`
- `GlobalTabs`

Normative rule:

- settings affect ordering and fallback, not semantic meaning of a selected result,
- ordering policy must remain explicit and test-covered.

---

## 5. Provider Suggestion Contract

### 5.1 Provider requests

Provider suggestions are fetched asynchronously.

Current behavior:

- provider suggestion requests are spawned off the hot UI path,
- parsed provider suggestion payloads are cached in runtime caches,
- network and parse failure become explicit provider status states.

### 5.2 Provider sources

Current provider templates:

- DuckDuckGo
- Bing
- Google

Default provider may be inferred from the configured search page.

Normative rule:

- provider suggestion behavior is opportunistic and non-blocking,
- provider failures must not prevent local/contextual matching or submission.

### 5.3 Standards relationship

External URL semantics govern:

- provider suggestion endpoint URL construction,
- submission URL normalization.

Graphshell remains the source of truth for:

- provider ordering,
- provider fallback,
- integration of provider suggestions with local/contextual matches.

---

## 6. Submission Arbitration Contract

The omnibar must arbitrate between:

- URL/navigation submission,
- provider search submission,
- contextual search selection,
- graph/node action result selection.

Normative rule:

- arbitration must be explicit and deterministic,
- selected match submission and raw text submission are separate paths,
- a chosen completion result must preserve its semantic kind through submission.

Current practical model:

- explicit result selection routes by result kind,
- raw text submission may resolve as URL, search page query, or scoped search based on existing submit helpers and search-page configuration.

Future tightening:

- if URL-vs-search heuristics are expanded, they must cite WHATWG URL behavior and document intentional deviations.

---

## 7. History Contract

The omnibar participates in history in at least three ways:

- input history / recent queries,
- navigation history submission outcomes,
- persisted frame/workspace restore paths that may set the field value indirectly.

Normative rule:

- input history and completion history are UI/session concerns,
- browser/navigation history remains a content/runtime concern,
- the omnibar must not collapse those into one undifferentiated history store.

Current code note:

- this area remains only partially explicit and should stay evolvable, but the separation of concerns must be preserved.

---

## 8. Action and Registry Relationship

The omnibar is a client of registry and routing systems, not an alternate command authority.

Normative rule:

- omnibar result selection may invoke registered actions or direct submission flows,
- but omnibar-specific ranking and arbitration live here, not in `ActionRegistry`,
- `ActionRegistry` still owns semantic action meaning where actions are involved.

---

## 9. Failure and Fallback Contract

Omnibar failure modes include:

- provider suggestion fetch failure,
- invalid/unsupported URL normalization,
- no local or global matches,
- rejected action/result kind,
- unavailable runtime target for submission.

Required behavior:

- failure must remain visible or diagnosable,
- provider failure must degrade gracefully to local/global behavior,
- no-result states must be representable without ambiguous silent no-op.

---

## 10. Diagnostics and Test Contract

Required coverage:

1. scope-prefix parsing,
2. provider-prefix parsing,
3. default provider inference from configured search page,
4. provider suggestion parsing and deduplication,
5. settings-driven non-`@` ordering presets,
6. connected-node ranking by contextual hop distance,
7. provider failure graceful degradation,
8. deterministic arbitration between selected-result submission and raw query submission.

Required diagnostics:

- provider fetch failure,
- malformed provider response,
- rejected submission route if applicable.

---

## 11. Acceptance Criteria

- [ ] omnibar parsing and ordering behavior are explicit and test-covered.
- [ ] provider suggestions remain asynchronous and non-blocking.
- [ ] local/contextual/global/provider sources remain distinct match families.
- [ ] URL/search/action arbitration remains explicit rather than implicit widget behavior.
- [ ] external standards guide URL/submission semantics, while Graphshell remains the source of truth for ranking and arbitration.
