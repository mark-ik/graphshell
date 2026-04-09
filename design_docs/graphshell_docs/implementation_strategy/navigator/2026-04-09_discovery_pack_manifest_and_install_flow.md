<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Discovery Pack Manifest and Install Flow

**Date**: 2026-04-09
**Status**: Implementation strategy / Track A follow-on
**Scope**: Define the user-facing and data-shape rules for opt-in discovery
packs that seed Graphshell with trusted or user-selected smolweb sources.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md)
- [2026-04-09_source_subscription_manager_and_health.md](2026-04-09_source_subscription_manager_and_health.md)
- [../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md](../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md)
- [../../research/2026-04-09_smolweb_discovery_and_aggregation_signal_model.md](../../research/2026-04-09_smolweb_discovery_and_aggregation_signal_model.md)

---

## 1. Purpose

Graphshell should not hardcode one canonical smolweb universe. It should allow
users to opt into discovery packs that seed:

- candidate sources,
- wayfinding surfaces,
- community hubs,
- example feeds and capsules.

Discovery packs are a user-visible onboarding and exploration feature, not a
hidden system default.

---

## 2. Manifest Minimum

Each pack manifest should declare at least:

- pack id,
- display name,
- description,
- source or curator provenance,
- version,
- item list,
- recommended lane tags,
- optional removal policy,
- optional update channel.

Each pack item should declare:

- canonical address or URL,
- item kind,
- label,
- optional description,
- discovery lane,
- optional tags,
- optional grouping/section hint.

---

## 3. Item Kinds

Discovery pack items may include:

- source candidates,
- subscription recommendations,
- wayfinding hubs,
- search engines,
- channel/community surfaces,
- exemplar documents or onboarding pages.

Important rule:

- installing a pack must not silently subscribe the user to everything in it
  unless the pack type explicitly says that is its behavior and the user opted
  into that mode.

Default interpretation:

- pack installation creates curated discovery candidates,
- the user decides which of those become actual subscriptions or saved sources.

---

## 4. Install Flow

The first install flow should be:

1. inspect pack metadata,
2. preview included items,
3. install as candidate sources,
4. optionally promote chosen items into subscriptions,
5. preserve pack provenance so later removal is reversible.

Pack install should be explicit and inspectable, not a hidden first-run seed.

---

## 5. Remove and Update Flow

Users should be able to:

- remove a pack while keeping explicitly subscribed sources,
- remove a pack and all still-pack-only candidates,
- inspect which sources came from the pack,
- refresh the pack definition when an update exists.

This keeps discovery packs from feeling like one-way imports.

---

## 6. Provenance Rules

Every candidate or source created by a discovery pack should retain:

- pack id,
- pack version,
- pack curator/source,
- install time,
- whether the user later promoted or edited the source manually.

That provenance is necessary for explainability and later cleanup.

---

## 7. Recommended Early Packs

Suitable early packs include:

- Bubble spaces,
- Cosmos-related sources,
- Wander consoles,
- curated Gemini feeds,
- trusted community hubs,
- learning packs for first-time smolweb exploration.

The goal is not to canonize one public web. The goal is to make opt-in
exploration easier.