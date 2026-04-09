<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Object Classification Model

**Date**: 2026-04-09
**Status**: Technical architecture / Track B follow-on
**Scope**: Define the durable object classes Graphshell needs so the graph can
distinguish source material, imported/public artifacts, user-authored material,
observations, and transient session state.

**Related**:

- [2026-04-09_identity_convergence_and_person_node_model.md](2026-04-09_identity_convergence_and_person_node_model.md)
- [2026-04-09_graphshell_verse_uri_scheme.md](2026-04-09_graphshell_verse_uri_scheme.md)
- [2026-04-09_browser_envelope_coop_and_degradation_policy.md](2026-04-09_browser_envelope_coop_and_degradation_policy.md)
- [../implementation_strategy/system/coop_session_spec.md](../implementation_strategy/system/coop_session_spec.md)
- [../implementation_strategy/social/comms/2026-04-09_irc_public_comms_lane_positioning.md](../implementation_strategy/social/comms/2026-04-09_irc_public_comms_lane_positioning.md)

---

## 1. Why This Exists

Graphshell cannot mature by treating every node as the same kind of thing.

The system needs to distinguish between:

- a source address,
- a fetched document snapshot,
- a user note,
- a person identity,
- a publication artifact,
- a discovery observation,
- a live co-op session object,
- a transient communication event.

Without these distinctions, storage, retention, routing, search, and policy all
blur together.

---

## 2. Classification Rule

Every durable or inspectable graph object should answer at least:

- what class of thing is this?
- what produced it?
- is it durable, reproducible, or transient?
- who owns or authored it?
- what retention policy applies?
- what URI or canonical identity should address it?

This classification is architectural, not merely cosmetic.

---

## 3. Core Object Classes

### 3.1 Address objects

Objects that identify a reachable external or internal location.

Examples:

- Gemini capsule address,
- feed URL,
- `verso://person/...`,
- `verso://tool/...`.

Important property:

- address objects are not the same thing as fetched content snapshots.

### 3.2 Content snapshot objects

Fetched or imported content in a specific observed state.

Examples:

- a gemtext document at fetch time,
- a JSON feed item as imported,
- a captured HTML document snapshot.

Important property:

- content snapshots are time-bound observations of content, not the address
  itself.

### 3.3 Identity objects

Objects that represent people, groups, or services as identity-bearing graph
actors.

Examples:

- person nodes,
- future community or room identities,
- service identities where appropriate.

Important property:

- identity objects may aggregate many external identifiers and endpoints.

### 3.4 Publication and message objects

Authored artifacts intended for publication, delivery, or transcripted
communication.

Examples:

- draft or published note,
- Misfin message,
- Titan submission artifact,
- future IRC or Matrix transcript item where retained.

Important property:

- publication/message objects are distinct from source snapshots even if they
  later become browsable content.

### 3.5 User-authored workspace objects

Local annotations and arrangement objects created by the user or collaborators.

Examples:

- note nodes,
- bookmarks,
- collections,
- workspace-local tags or grouping objects,
- graph arrangement metadata.

### 3.6 Observation and signal objects

Objects or edges representing discovery, freshness, search, ranking, relation
inference, or provenance observations.

Examples:

- "discovered via pack X",
- "observed stale at time T",
- neighborhood-walk candidate relation,
- inferred same-person linkage.

Important property:

- these are not necessarily authored content and should not pretend to be.

### 3.7 Session and presence objects

Live co-op or runtime state with limited retention semantics.

Examples:

- active co-op participant,
- cursor/presence state,
- temporary room membership,
- current shared focus state.

Important property:

- session objects often have stricter retention and host-envelope constraints.

---

## 4. Required Policy Axes

Each object class should be evaluable along these policy axes:

- durability,
- mutability,
- retention,
- authorship,
- provenance,
- shareability,
- host-envelope availability,
- URI/addressability.

These axes matter more than purely visual labels.

---

## 5. Practical Consequences

This model should drive:

- which actions appear in command surfaces,
- which audit/provenance details appear in inspectors,
- what can be exported or published,
- what can be safely synced or shared,
- what history and branching semantics apply,
- what should be searched as content versus indexed as source or signal.

---

## 6. Near-Term Requirement

The first implementation pass does not need every future object class, but it
does need the system to stop collapsing all imported things into one generic
node notion.

Minimum useful split:

1. address,
2. content snapshot,
3. identity,
4. publication/message,
5. user-authored workspace object,
6. observation/signal,
7. session/presence.

That is the floor required for the next stage of Middlenet and co-op design.

---

## 7. Implementation Slices

### Slice A: Canonical Classification Enum and Metadata Axes

- add a canonical object-class inventory,
- define the required metadata axes for durability, provenance, retention,
  authorship, and addressability,
- ensure the inventory is usable across graph storage, routing, and inspectors.

### Slice B: Imported Source vs Snapshot Split

- separate source/address objects from fetched content snapshots,
- keep imported content time-bound and provenance-bearing,
- stop overloading one generic node notion for both roles.

### Slice C: Identity, Publication, and Signal Distinctions

- make person and future community/service identity objects explicit,
- separate publication/message artifacts from both source snapshots and local
  notes,
- add observation/signal records or edge classifications for discovery and
  freshness provenance.

### Slice D: Session and Presence Policy

- classify live co-op/session state separately from durable content,
- apply stricter retention and sharing policy,
- ensure host-envelope constraints can be attached to this class cleanly.

---

## 8. Validation

### Architecture

- every inspectable object class has a clear retention and provenance story,
- address objects and content snapshots are not conflated,
- session/presence state is not accidentally treated as durable publication or
  note content.

### Implementation

1. Inspect imported content and verify source identity, snapshot identity, and
   signal provenance are all distinguishable.
2. Inspect a person or message artifact and verify it does not route through
   generic page assumptions.
3. Inspect live co-op or presence state and verify it carries explicit
   retention limits.

---

## 9. Done Gate

This note is operationalized when:

- the system has a canonical object-class inventory,
- at least the minimum seven-way split is reflected in storage or metadata
  policy,
- command surfaces and inspectors can differentiate those classes,
- and future Middlenet/co-op work no longer depends on one undifferentiated
  node model.