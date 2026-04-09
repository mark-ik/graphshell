<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Constellation Projection Plan

**Date**: 2026-04-09
**Status**: Implementation strategy / Track A follow-on
**Scope**: Define the first Navigator specialty projection for thread-like
smolweb local worlds inspired by Cosmos and related cross-source discussions.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md)
- [../../technical_architecture/graphlet_model.md](../../technical_architecture/graphlet_model.md)
- [../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md](../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md)
- [../../research/2026-04-09_smolweb_discovery_and_aggregation_signal_model.md](../../research/2026-04-09_smolweb_discovery_and_aggregation_signal_model.md)

---

## 1. Purpose

Graphshell needs a Navigator projection that treats thread-like smolweb
structures as bounded local worlds instead of as flat lists of pages.

This projection is called a **constellation projection**.

It is especially motivated by:

- Cosmos-like thread grouping,
- Bubble reply/reference structures,
- gemlog citation and response chains,
- future feed/post/reply groupings across sources.

---

## 2. Canonical Shape

A constellation projection is a graphlet-oriented Navigator projection defined
by:

- one anchor item,
- reply/reference relations,
- optional secondary related items,
- a frontier of candidate expansions,
- recency and relevance metadata.

The goal is not only to show chronological order. The goal is to let the user
understand the **local world of a discussion or related cluster**.

---

## 3. Required Relation Types

The first slice needs explicit relation families for:

- reply-to,
- references,
- cites/mentions,
- same-thread or same-constellation membership,
- frontier candidate relation.

These do not all need to be durable graph truth on day one, but the projection
must be able to distinguish them.

---

## 4. Required User Questions

The first projection should answer:

- what is the anchor item?
- what directly replies to it?
- what is referenced by it?
- what else belongs to this cluster?
- what is newer, adjacent, or likely worth opening next?

If it cannot answer these, it is not yet a constellation view, only a list.

---

## 5. Presentation Rules

The first presentation should support:

- anchor emphasis,
- relation-aware ordering,
- frontier ranking,
- compact cluster layout,
- clear separation between current cluster members and candidate expansions.

The first slice does not need one final visual language, but it does need a
specialty layout better than a generic feed list.

---

## 6. Input Sources

The projection should be able to ingest from:

- Cosmos imports,
- Bubble-style posts and issues,
- feed items with reply/reference metadata,
- future Nostr or Matrix bridges where a thread-like local world exists.

This keeps the feature conceptually broader than any one external service.

---

## 7. Near-Term Slice

Recommended first slice:

1. one anchor item,
2. typed reply/reference relations,
3. frontier candidate model,
4. Navigator projection state for the active constellation,
5. open/reveal/scope verbs consistent with Navigator grammar.

This is enough to validate whether the feature genuinely helps orientation.