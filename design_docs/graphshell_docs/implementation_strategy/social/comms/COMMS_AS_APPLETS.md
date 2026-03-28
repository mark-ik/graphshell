<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Comms As Applets

**Date**: 2026-03-28
**Status**: Draft / canonical direction
**Scope**: Places Comms in Graphshell's social-domain implementation strategy as a hosted surface family that composes multiple communication lanes without becoming a separate protocol root or core semantic domain.

**Related docs**:

- [`../../technical_architecture/GRAPHSHELL_AS_BROWSER.md`](../../technical_architecture/GRAPHSHELL_AS_BROWSER.md) — Graphshell as host/renderer rather than protocol owner
- [`../../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — Verso bilateral boundary and local-first host model
- [`../../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md`](../../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md) — Verse community-scale network boundary
- [`../../../matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md`](../../../matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md) — Matrix as the durable room substrate
- [`../../../nostr_docs/implementation_strategy/2026-03-05_nostr_mod_system.md`](../../../nostr_docs/implementation_strategy/2026-03-05_nostr_mod_system.md) — Nostr as a cross-cutting social capability fabric
- [`../../../verso_docs/implementation_strategy/coop_session_spec.md`](../../../verso_docs/implementation_strategy/coop_session_spec.md) — co-op as a Verso bilateral session capability, not the Comms root model

---

## 1. Decision Summary

**Comms** is not a separate top-level protocol or semantic domain.

It is an **optional Graphshell-hosted applet/surface family** that can be invoked inside Graphshell panes and can compose multiple transport or identity backends without collapsing them into one network model.

Comms surfaces may include:

- durable room chat and participation lanes hosted by Matrix
- public or relay-oriented social/chat lanes hosted by Nostr primitives
- bilateral session chat and co-presence lanes hosted by Verso over iroh
- future mixed communication shells that present these lanes under one host surface

Graphshell hosts those surfaces. Hosting does not make the shell the owner of the underlying communication semantics.

---

## 2. Ownership Boundary

- **Graphshell** owns hosting, invocation, pane/workbench placement, and reducer/workbench authority boundaries for Comms surfaces.
- **Verso** owns bilateral communication/session behavior such as co-op and any session-local chat or presence lanes over iroh.
- **MatrixCore** owns durable room membership, room state, and room-scoped communication semantics.
- **NostrCore** owns relay-facing public/social messaging, identity, and publication semantics.
- **Verse** may host community-scale communication surfaces, but does not absorb the bilateral or room-specific semantics of those lower layers.

The practical rule is simple: Comms is the presentation family that can compose multiple lanes; it is not a replacement protocol and not a new semantic root beneath the existing mods.

---

## 3. Documentation Placement Rule

Use `graphshell_docs/implementation_strategy/social/` for docs whose main subject is the **hosted communication surface model itself**:

- unified chat shells
- cross-lane channel rails
- applet-level communication UX
- surface-level coordination rules across Matrix, Nostr, and Verso co-op lanes

Keep protocol- or transport-authority docs in their owning directories:

- `verso_docs/` for bilateral session/co-op authority
- `matrix_docs/` for durable room authority
- `nostr_docs/` for relay/social capability authority
- `verse_docs/` for community-scale network authority

---

## 4. Non-Goals

- Comms does not redefine co-op as a Graphshell-core subsystem.
- Comms does not merge Matrix rooms, Nostr channels, DMs, and Verso bilateral chat into one protocol.
- Comms does not own identity binding, room moderation, relay policy, or bilateral transport. Those remain in the relevant mod-specific authorities.