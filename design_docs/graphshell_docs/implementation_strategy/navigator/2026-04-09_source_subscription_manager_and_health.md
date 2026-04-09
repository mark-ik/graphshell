<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Source Subscription Manager and Health

**Date**: 2026-04-09
**Status**: Implementation strategy / Track A follow-on
**Scope**: Define the user-visible subscription, source inventory, and
source-health surface for Middlenet discovery and feed operation.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md)
- [navigator_backlog_pack.md](navigator_backlog_pack.md)
- [../../research/2026-04-09_smolweb_browser_capability_gaps.md](../../research/2026-04-09_smolweb_browser_capability_gaps.md)
- [../../research/2026-04-09_smolweb_discovery_and_aggregation_signal_model.md](../../research/2026-04-09_smolweb_discovery_and_aggregation_signal_model.md)
- [../../technical_architecture/2026-04-09_graphshell_verse_uri_scheme.md](../../technical_architecture/2026-04-09_graphshell_verse_uri_scheme.md)

---

## 1. Why This Exists

Supporting feeds or source imports is not the same thing as operating a source
browser well. Graphshell needs a first-class surface where the user can answer:

- what am I following?
- what changed recently?
- what is stale, empty, broken, or redirected?
- which sources are only discovery candidates and which are actual
  subscriptions?
- why is this source present here at all?

This note defines that surface.

---

## 2. Core Position

The Source Subscription Manager is a **Navigator-owned operational surface**.

It reads from:

- source nodes,
- user subscription/follow state,
- discovery provenance,
- recency and source-health signals,
- saved/offline-reading state where relevant.

It does not own protocol truth or content rendering. It is the browser
operations layer for keeping sources legible and actionable.

---

## 3. Source States

Every source should be classifiable along at least two axes.

### 3.1 Relationship state

- `candidate`
- `subscribed`
- `muted`
- `archived`

Important rule:

- a discovery candidate is not the same thing as a subscription,
- conversion from candidate to subscribed must be explicit,
- archived or muted sources remain inspectable rather than vanishing.

### 3.2 Health state

- `healthy`
- `stale`
- `empty`
- `redirected`
- `broken`
- `unknown`

Health must be based on explicit observations and timestamps, not vague UI
impressions.

---

## 4. Minimum Surface Requirements

The first Source Subscription Manager slice should show:

- source title and canonical address,
- source type or lane,
- subscription state,
- most recent successful update timestamp,
- recent entry count or known empty state,
- health state,
- last failure or redirect status when relevant,
- provenance of how the source was discovered.

This is the minimum required to make subscriptions feel operable rather than
passive.

---

## 5. Required Operations

The first slice should support:

- subscribe / unsubscribe,
- mute / unmute,
- archive / unarchive,
- refresh source,
- inspect source provenance,
- open source,
- open recent content from source,
- remove discovery candidate,
- promote discovery candidate into subscription.

Later operations may include grouping, tagging, pack membership management, and
shared/community source lists.

---

## 6. Provenance and Explainability

For every source, the user should be able to inspect:

- where it came from,
- which signal lane surfaced it,
- whether it came from a discovery pack, manual add, search result, imported
  relation, or neighborhood walk,
- when it was last checked,
- why it is considered stale or broken.

This surface is where Graphshell makes "why is this feed broken?" or "why is
this source here?" answerable.

---

## 7. Saved and Offline Reading Relationship

The Source Subscription Manager should not own saved-reading truth, but it
should surface enough state to keep source operation and retention legible.

Useful fields include:

- has saved items,
- has offline-reading material,
- retention policy or status summary,
- whether recent content is only live, partially cached, or explicitly saved.

This keeps subscription management aligned with the saved/offline-reading model
without collapsing them into one concept.

---

## 8. Recommended First Slice

1. source-node and subscription-state schema,
2. health-state computation rules,
3. refresh and failure visibility,
4. candidate vs subscribed split,
5. provenance panel,
6. recent-update listing per source.

This gives Graphshell a real browser-operations floor before broader protocol
expansion.

---

## 9. Implementation Slices

### Slice A: Source Object and Relationship State

- define source-node fields needed by Navigator,
- add explicit relationship state for candidate, subscribed, muted, and
  archived,
- keep relationship state separate from health observations.

### Slice B: Health Computation and Refresh Visibility

- compute `healthy`, `stale`, `empty`, `redirected`, `broken`, and `unknown`
  from explicit observations,
- surface last successful refresh, failure, or redirect state,
- route refresh through a canonical action surface.

### Slice C: Provenance and Recent-Update Surface

- show why a source exists,
- show which signal lane or discovery pack introduced it,
- provide a recent-update list or count tied to that source.

### Slice D: Candidate Promotion and Cleanup

- support promote candidate to subscribed,
- support remove candidate without breaking durable subscriptions,
- preserve provenance for later audit and pack cleanup.

---

## 10. Validation

### Manual

1. Add a discovery candidate and verify it appears as `candidate`, not
  `subscribed`.
2. Promote that source into a subscription and verify the state change is
  explicit and reversible.
3. Refresh a source and verify health fields and timestamps change without
  corrupting provenance.
4. Inspect a stale or broken source and verify the reason is visible.

### Automated

- state-model tests for relationship and health transitions,
- provenance tests proving lane and pack metadata persist across refreshes,
- UI or reducer tests for promote/remove/archive/mute flows.

---

## 11. Done Gate

This slice closes when:

- source relationship state and health state are both explicit,
- the user can promote, mute, archive, refresh, and inspect sources,
- provenance and recent-update context are visible,
- and discovery candidates are no longer conflated with subscriptions.