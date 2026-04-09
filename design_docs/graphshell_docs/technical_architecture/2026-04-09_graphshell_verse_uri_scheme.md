<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graphshell / Verse URI Scheme Baseline

**Date**: 2026-04-09
**Status**: Architectural baseline aligned to current parser behavior
**Purpose**: Define the canonical internal/external address space for Graphshell
and adjacent Verse-facing artifacts so that address forms stop accreting ad hoc.

**Related docs**:

- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
- [`2026-04-09_identity_convergence_and_person_node_model.md`](2026-04-09_identity_convergence_and_person_node_model.md)
- [`../research/2026-03-30_middlenet_vision_synthesis.md`](../research/2026-03-30_middlenet_vision_synthesis.md)

---

## 1. Canonical Scheme

The canonical internal Graphshell/Verse scheme is:

- `verso://`

Compatibility aliases:

- `graphshell://` is accepted as a compatibility alias for the same internal
  address space.

Legacy adjacent schemes still exist in code for narrower purposes:

- `graph://`
- `node://`
- `notes://`

These should be treated as legacy or specialty address types, not as rivals to
the canonical `verso://` space.

---

## 2. Current Baseline (2026-04-09)

The current parser/formatter already supports these `verso://` categories:

- `settings`
- `frame`
- `tile-group`
- `view`
- `tool`
- `clip`
- `other` categories preserved opaquely

The current codebase also already uses `verso://person/<id>` style addresses
via the generic `Other { category, segments }` path, which means person-node
and person-artifact addressing already fits naturally inside the canonical
scheme.

This doc therefore does not invent a new URI family. It formalizes the one the
repository is already converging toward.

---

## 3. Address Model

`verso://` addresses are:

- stable graph/workbench identifiers,
- not raw transport locators,
- suitable for local routing, sharing, and future portable envelopes,
- open to additional categories without breaking the parser.

Rule:

- unknown categories should be preserved rather than rejected when possible,
- canonical rendering should emit `verso://`, even if a compatibility alias was
  parsed as input.

---

## 4. Current Canonical Forms

Current forms supported directly by parser behavior:

- `verso://settings`
- `verso://settings/history`
- `verso://frame/<frame-id>`
- `verso://tile-group/<group-id>`
- `verso://view/<legacy-view-id>`
- `verso://view/graph/<graph-id>`
- `verso://view/node/<node-id>`
- `verso://view/note/<note-id>`
- `verso://tool/<name>`
- `verso://tool/<name>/<instance>`
- `verso://clip/<clip-id>`
- `verso://person/<person-id>`
- `verso://person/<person-id>/<artifact-kind>/<artifact-id>`

These are enough to cover:

- internal workbench routing,
- person nodes and their artifacts,
- settings/tool surfaces,
- future internal share targets.

---

## 5. Reserved Near-Term Categories

The following categories should be treated as reserved near-term extensions of
the same scheme:

- `session`
- `room`
- `community`
- `cabal`
- `workspace`
- `snapshot`
- `publication`

These categories are reserved because current design docs already imply them,
and the goal of this note is to stop them from emerging as ad hoc, mutually
incompatible mini-schemes.

---

## 6. Compatibility and Normalization Rules

Normalization rules:

1. `verso://` is canonical on output.
2. `graphshell://` is accepted on input for compatibility.
3. Query strings and fragments may carry UI hints, but the routed canonical
   address is the path/category/segment identity.
4. Category tokens should be lowercase.
5. Segment case should be preserved unless a category-specific rule says
   otherwise.

Compatibility rule:

- Graphshell should prefer extending `verso://` with new categories over
  inventing additional top-level internal schemes.

---

## 7. Scope Boundary

This URI space is for:

- internal graph/workbench addresses,
- portable share targets,
- person/publication/session/community addresses that Graphshell may need to
  exchange across hosts later.

It is **not** a replacement for:

- external web/smallnet transport URLs,
- Gemini, Gopher, HTTP, or other protocol locators,
- opaque peer/network protocol internals that do not need user-facing address
  forms.

---

## 8. Remaining Open Work

The next unresolved URI-policy items are:

1. co-op/session invite forms,
2. room/community/cabal address semantics,
3. workspace/snapshot/publication portability guarantees,
4. how much of `graph://`, `node://`, and `notes://` should remain public vs.
   be treated as legacy compatibility layers only.

The immediate win from this note is simpler: Graphshell now has a canonical
answer to "which internal scheme should new addressable things use?" The answer
is `verso://`.