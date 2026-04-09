<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# IRC Public Comms Lane Positioning

**Date**: 2026-04-09  
**Status**: Draft / positioning note  
**Scope**: Place IRC inside Graphshell's hosted Comms surface family as a public
community communication lane for smolweb-adjacent spaces such as tildeverse IRC
networks.  
**Out of scope**: Turning Graphshell into a full-featured IRC power client;
replacing Matrix, Cable, or Nostr; mandatory transcript retention; bot/operator
tooling; DCC/file transfer; bouncer management; exhaustive IRCv3 coverage.

**Related docs**:

- [`COMMS_AS_APPLETS.md`](COMMS_AS_APPLETS.md) — Comms as hosted surface family
- [`../../../../verso_docs/implementation_strategy/2026-03-28_cable_coop_minichat_spec.md`](../../../../verso_docs/implementation_strategy/2026-03-28_cable_coop_minichat_spec.md) — bilateral/co-op minichat positioning
- [`../../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md`](../../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md) — smolweb research note including IRC relevance
- [`../../../technical_architecture/GRAPHSHELL_AS_BROWSER.md`](../../../technical_architecture/GRAPHSHELL_AS_BROWSER.md) — host/graph/workbench/viewer split
- [`../../../implementation_strategy/subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`](../../../implementation_strategy/subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md) — privacy and retention boundaries

**External references**:

- [tilde.chat](https://tilde.chat/) — public tildeverse IRC network with TLS connection info and webchat examples

---

## 1. Decision Summary

IRC is a good candidate for Graphshell, but only if it is positioned correctly.

The correct position is:

- **IRC is a public communication lane inside Comms**, not a new top-level
  Graphshell protocol root.
- **Graphshell hosts IRC surfaces**, but does not become the owner of IRC as a
  protocol ecosystem.
- **The first slice should optimize for "just works" public/community use**,
  not for exhaustive parity with dedicated IRC clients.

In practical terms, this means Graphshell may host an IRC applet for spaces such
as `tilde.chat`, but should not initially aim to replace:

- WeeChat
- irssi
- HexChat
- dedicated mobile IRC clients
- bouncers and operator tooling

The point of the IRC lane is not "be the best IRC client". The point is:

- let the user participate in smolweb-adjacent community spaces from inside the
  same environment where they browse, collect, clip, and graph content,
- make links, channel references, and selected excerpts legible as graph inputs,
- keep retention and archival strictly opt-in and bounded.

---

## 2. Why IRC Fits

IRC remains relevant because a meaningful part of the small-web/social-unix
world still uses it as a real communication substrate. `tilde.chat` is a clear
example: it is the IRC network of the tildeverse, publishes straightforward TLS
connection information, and offers webchat as an accessibility/on-ramp rather
than as the only expected entry point.

For Graphshell, that matters because IRC is not an isolated "legacy protocol".
It intersects with:

- the tildeverse and other social-unix communities,
- public channels that function as orientation and support surfaces,
- live link-sharing and discussion around smolweb resources,
- later archival or excerpt workflows when the user intentionally keeps what was
  actually served to them.

This makes IRC a natural complement to:

- Matrix for durable room/state spaces,
- Cable for small-group bilateral/co-op chat,
- Nostr for public relay/social publication lanes.

The lanes are related, but not interchangeable.

---

## 3. Ownership Boundary

IRC should follow the same applet/surface discipline already established for
Comms.

- **Graphshell** owns hosting, invocation, pane placement, workbench routing,
  selection handoff, and graph capture affordances around the IRC surface.
- **The IRC lane** owns IRC-session behavior for the limited supported feature
  set: connection, join, message stream, topic display, user roster, and basic
  message send/receive.
- **Graph truth** does not become raw chat truth. Links, selected excerpts,
  channels, servers, and optionally user-approved artifacts may become graph
  material through explicit user action.

This preserves the key boundary:

- chat remains chat,
- graph capture remains graph capture,
- hosting does not imply semantic ownership.

---

## 4. First Slice

The first slice should be intentionally small.

### 4.1 Included

- connect to an IRC server over TLS
- join one or more channels
- render channel message streams
- render channel topic and basic roster/presence information
- send ordinary messages
- open links from messages through the normal Graphshell browsing pipeline
- create explicit graph artifacts from selected links or excerpts
- optionally keep a **local session transcript**, off by default

### 4.2 Excluded

- DCC/file transfer
- full IRCv3 capability matrix
- bot administration
- operator tools
- bouncer setup/management UX
- automatic import of historical backlogs from third-party infrastructure
- automatic transcript retention
- silent archival of public channels

This slice is enough to support the "public smolweb community lane" use case
without opening a large product surface all at once.

---

## 5. Runtime Feasibility

Graphshell's current dependency/runtime shape suggests that IRC is not blocked
by missing fundamentals.

The repository already carries async/networking pieces such as:

- `tokio`
- `tokio-rustls`
- `futures-util`

That means the main difficulty is not "can Graphshell open a TLS socket and
stream lines?" The main difficulty is product discipline:

- defining the supported IRC feature subset,
- integrating the surface into Comms without scope creep,
- keeping graph capture explicit,
- keeping retention consent-bound.

This is a strong reason to attempt IRC only as a narrow lane with a clear done
gate.

---

## 6. Retention and Privacy Boundary

This lane must follow a strict retention rule.

Graphshell should only save:

- material actually served to the user, and/or
- material whose participants have explicitly consented to saving.

Consequences:

- no default transcript retention,
- no silent channel scraping,
- no automatic dataset generation from public chat,
- no implied right to retain because a channel is public,
- local transcript export must be a deliberate user action or an explicit
  session preference.

If transcripts are enabled, the UI should make that state obvious.

This keeps the IRC lane aligned with the broader privacy boundary already
surfaced in the smolweb research note and subsystem-security work.

---

## 7. Graphshell-Specific Value

Graphshell adds value to IRC when it does things dedicated clients usually do
not prioritize:

- **link-centric browsing handoff**: URLs in channels open directly into the
  graph/browser/workbench flow
- **explicit excerpt capture**: selected messages or snippets can become notes,
  citations, or references
- **cross-surface context**: a channel can live alongside related docs, feeds,
  and graphlets
- **community wayfinding**: support/help/community channels can be treated as
  first-class orientation surfaces rather than external tools

This is the "just works" benefit:

- the user does not need one app for Gemini, one for feeds, one for notes, one
  for graph capture, and another one just to check the community room where the
  same links are being discussed.

The trick is to stop at this benefit and not absorb every advanced IRC workflow.

---

## 8. Non-Goals and Guardrails

The IRC lane should not become:

- the canonical durable room model
- the preferred small-group private chat substrate
- a protocol-unification project
- an excuse to blur public channel text and graph truth into one store

Guardrails:

1. Public IRC stays a **Comms lane**, not a cross-app authority.
2. Capture into graph truth is always explicit.
3. Retention is always opt-in.
4. Advanced IRC operations remain out of scope until the basic lane proves
   value.

---

## 9. Recommended Next Step

The recommended next step after this positioning note is a short execution plan
for an IRC applet MVP with:

- TLS connect/join/send/receive
- channel surface model
- link activation into Graphshell
- opt-in local transcript toggle
- explicit "capture message/link to graph" actions
- a narrow validation matrix based on one public network such as `tilde.chat`

That would be enough to validate whether IRC genuinely improves the
smolweb/community experience in Graphshell without committing the project to a
full IRC-client roadmap.
