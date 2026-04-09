<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Middlenet Vision Synthesis

**Date**: 2026-03-30  
**Status**: Research synthesis / architectural critique consolidation  
**Purpose**: Consolidate the current Graphshell "middlenet browser" vision, a broad protocol survey, and multiple rounds of critique into one usable research document instead of a transcript bundle.

**Related docs**:

- [`../technical_architecture/2026-03-29_middlenet_engine_spec.md`](../technical_architecture/2026-03-29_middlenet_engine_spec.md)
- [`../technical_architecture/2026-03-29_portable_web_core_host_envelopes.md`](../technical_architecture/2026-03-29_portable_web_core_host_envelopes.md)
- [`../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md`](../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md)
- [`../technical_architecture/GRAPHSHELL_AS_BROWSER.md`](../technical_architecture/GRAPHSHELL_AS_BROWSER.md)
- [`../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md`](../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md)
- [`../../verso_docs/research/2026-03-28_smolnet_follow_on_audit.md`](../../verso_docs/research/2026-03-28_smolnet_follow_on_audit.md)
- [`../../verso_docs/research/2026-03-28_smolnet_dependency_health_audit.md`](../../verso_docs/research/2026-03-28_smolnet_dependency_health_audit.md)
- [`../implementation_strategy/system/2026-03-05_network_architecture.md`](../implementation_strategy/system/2026-03-05_network_architecture.md)
- [`../../verso_docs/implementation_strategy/coop_session_spec.md`](../../verso_docs/implementation_strategy/coop_session_spec.md)
- [`../../matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md`](../../matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md)

---

## 1. Why This Doc Exists

Several adjacent inputs now exist:

- the project-owner statement of vision for Graphshell as a middlenet browser,
- an initial critique of that vision,
- a broad "small internet" protocol inventory,
- a more naive but still useful architecture response,
- a more targeted gap analysis focused on protocols, identity, and host-envelope realities.

Those inputs contain real insight, but they also mix together:

- current Graphshell commitments,
- plausible future directions,
- niche protocol curiosities,
- practical host-platform constraints,
- speculative ideas that should not silently become roadmap commitments.

This document separates those layers.

Some conclusions first surfaced here have now been promoted into canonical
architecture policy in
[`2026-03-30_protocol_modularity_and_host_capability_model.md`](../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md).
This file should remain a synthesis and critique surface, not a rival policy
authority.

---

## 2. Source Vision Summary

The current high-level vision can be summarized as follows:

- Graphshell becomes a **middlenet browser**: a graph-first browser for the smallnet, middle web, and selected parts of the structured modern web.
- The **graph is the primary session truth**. The tile tree and viewers are projections or attachments derived from that graph truth.
- Each node can open into a pane/tile, but nodes and edges are also **rich persistent data objects** carrying metadata, history, and event-log meaning.
- The project prefers **secure smallnet protocols** where overlap exists, but still supports plaintext protocols faithfully.
- The long-term browser stack is intended to be **portable across desktop, mobile, extension, and browser-tab/PWA hosts**, with the same conceptual engine reused across host envelopes.
- The collaborative/networked dimension includes:
  - bilateral sync and co-op,
  - room-style durable collaboration,
  - portable user identity,
  - distributed storage and archival,
  - community-governed discovery and trust.

In short: Graphshell is not trying to become "yet another browser with a graph view." It is trying to make the graph itself the primary browser memory, social surface, and portable knowledge substrate.

---

## 3. What The Existing Architecture Already Gets Right

The current docs already establish several strong and unusually coherent positions.

### 3.1 Graph truth vs workbench truth vs viewer truth

This is one of the clearest strengths of the project.

- [`GRAPHSHELL_AS_BROWSER.md`](../technical_architecture/GRAPHSHELL_AS_BROWSER.md) already states that pages/resources are graph-backed nodes, history is temporal rather than only back/forward, pane arrangement is workbench-owned, and viewer lifecycle is reconcile-driven.
- This is the right foundation for a graph browser. It prevents the graph from collapsing into "fancy tab chrome."

### 3.2 One core, many hosts

The portable host-envelope model is sound.

- [`2026-03-29_portable_web_core_host_envelopes.md`](../technical_architecture/2026-03-29_portable_web_core_host_envelopes.md) clearly adopts the mental model of **one singular portable web/document core** reused by many host envelopes.
- That avoids the common mistake of treating native app, extension, and web build as separate products that merely share branding.

### 3.3 Middlenet as a content class, not a new protocol

The middlenet framing is productively modest.

- [`2026-03-29_middlenet_engine_spec.md`](../technical_architecture/2026-03-29_middlenet_engine_spec.md) correctly defines MiddleNet as an observation about a class of content rather than a new protocol.
- The shared intermediate document model is the right architectural anchor for Gemini/Gopher/Finger/RSS/Markdown/static HTML.
- **The intermediate document model is a rendering AST, not a user-facing format.** Each source format is parsed into this internal tree; the tree is a rendering target, not a richer protocol output.
- **Each source format is rendered as itself.** Gemini content renders as gemtext; Gopher renders as a Gopher menu or document; Markdown renders as Markdown. The shared model is the internal parse target, not a reason to enrich or homogenize the output of any individual format. Gemtext's intentional minimalism is a deliberate design stance and should be respected.

### 3.4 Markdown as the authored-content format

When Graphshell itself needs to author content — graph node annotations,
published artifacts, co-op shared documents — the format is **Markdown**
(CommonMark base). This is already a first-class document lane and familiar to
most users.

Conservative Graphshell-specific extensions (e.g. graph-link syntax) may be
added where there is clear product need, but the base is standard CommonMark.
This is not a new protocol; it is a content-format decision for authored
content that Graphshell itself produces or stores.

This keeps authored content distinct from browsed content and prevents
"MiddleNet document format" from silently becoming a new protocol invention.

### 3.5 Strong network layer separation

The network architecture is already better-specified than many projects at this stage.

- [`2026-03-05_network_architecture.md`](../implementation_strategy/system/2026-03-05_network_architecture.md) distinguishes three contextual substrates:
  - bilateral: iroh,
  - room: Matrix,
  - community: libp2p/Verse.
- It also correctly treats Nostr as a social capability fabric and WebRTC as a media/signaling capability rather than pretending all protocols do the same job.

### 3.6 Co-op scope discipline

The co-op model is usefully constrained.

- [`coop_session_spec.md`](../../verso_docs/implementation_strategy/coop_session_spec.md) keeps one host per session, keeps workbench layout local, and limits host authority to the shared co-op surface.
- That is a strong product decision because it stops live collaboration from erasing the personal/local nature of the graph workspace.

---

## 4. Main Architectural Risks And Missing Boundaries

The biggest remaining gaps are mostly **boundary gaps**, not "missing cool features."

### 4.1 Scope collapse across Graphshell, Verso, and Verse

The current vision spans:

- graph browser,
- portable document engine,
- smallnet suite,
- collaborative browsing layer,
- room substrate,
- identity fabric,
- distributed storage bank,
- community discovery/governance layer.

This is compelling, but it risks turning one project into six interlocked projects.

Recommended boundary:

- **Graphshell**: graph UX, workbench, viewer routing, local truth, browser behavior.
- **Verso**: bilateral peer agent, web capability host, co-op, protocol-facing peer features.
- **Verse**: community-scale trust, storage, federation, publication, and governance.

These should remain separate in both docs and roadmap sequencing.

### 4.2 Insufficiently explicit graph object classes

The graph currently wants to hold:

- fetched content,
- authored notes,
- traversal history,
- shared/public artifacts,
- ephemeral collaboration state,
- inferred metadata,
- cross-protocol identity links.

That only scales if Graphshell explicitly distinguishes at least:

- content snapshot,
- source address,
- user annotation,
- traversal/event edge,
- inferred relation,
- shareable publication artifact,
- ephemeral presence/session event.

Otherwise the graph becomes semantically rich but operationally fuzzy.

### 4.3 Trust and identity are under-modeled as user-facing product surfaces

Secure-vs-plaintext preference is present in the Middlenet spec, but the broader trust model needs a fuller UI and data model.

Missing or underspecified concerns:

- TOFU lifecycle and certificate rotation,
- Gemini client certificate identity UX,
- sender identity binding for Misfin,
- NIP-05 and WebFinger convergence,
- explicit binding between Nostr, Matrix, Gemini/Misfin, and domain identities,
- presentation of plaintext protocols as neutral-but-explicit rather than panic-inducing.

This project is building a browser for trust-rich, protocol-diverse spaces. That means trust is not a side panel concern.

### 4.4 Merge model for community/shared graph truth

Co-op has a host-led approval model. That is good for live sessions.

But the longer-lived shared/community layer still needs a declared merge discipline:

- approval queue,
- CRDT,
- branch/merge,
- append-only proposal log with curator acceptance,
- or a hybrid.

Without that, Verse risks becoming strong on transport and storage while weak on communal knowledge editing.

### 4.5 Discovery and ranking are less mature than storage and transport

The project is strongest today on:

- rendering,
- host portability,
- protocol layering,
- co-op transport boundaries,
- storage/economic thought.

It is weaker on:

- shared indexing,
- result ranking,
- trust-weighted discovery,
- community-specific relevance models,
- how "find what matters" actually works from inside the graph.

This matters because the user-facing promise is not merely "we can store and sync graph objects." It is "communities and users can decide what is relevant."

---

## 5. Practical Constraint: The WebRender Gate

One especially important reality surfaced in the critique:

- the broader full-web browser vision is still heavily gated by the
  WebRender/wgpu work, even though the Middlenet document/transport lane can
  proceed independently.

This does not invalidate the architecture, but it does imply a sequencing truth:

- near-term Graphshell should behave like a **graph browser with strong smallnet/document lanes and selective full-web delegation**,
- not like a nearly-finished general-purpose browser whose last step is optimization.

The more honest product framing in the near term is:

- best-in-class Tier 0/1 browsing,
- reader-grade/document-grade middlenet browsing,
- graph-first knowledge capture and routing,
- optional full-web delegation through Servo/Wry/host browser where needed.

---

## 6. Protocol Inventory: Strategic Reading

The supplied protocol inventory is broader than Graphshell should implement, but it is useful as landscape mapping.

### 6.1 High-value core smallnet lanes

These align strongly with the current architecture, but they do not all belong
in the same product surface.

Core document/read lanes:

- Gemini
- Gopher
- Finger
- Spartan
- Nex
- RSS / Atom
- Markdown / plain text
- static HTML

Adjacent mutation/messaging lanes:

- Titan
- Misfin

These either already appear in current docs or are close extensions of
existing design logic, but Titan and Misfin should remain mutation/messaging
surfaces rather than being folded into the passive document renderer.

### 6.2 Technically interesting but likely low-priority lanes

These may be worth documenting as "known but deferred" rather than treating as active candidates:

- Mercury
- Scorpion
- Text Protocol
- Scroll
- Gopher+
- Gopher over TLS
- Guppy
- FSP
- Molerat
- SuperTXT
- TerseNet

Some are architecturally interesting; most are ecosystem-niche.

### 6.3 Adjacent receivable/federated protocols worth explicit note

These are not "smallnet" in the strict sense, but they matter because they touch Graphshell's stated goals:

- WebFinger
- NIP-05
- ActivityPub
- WebMention
- JSON Feed
- WebSub
- BitTorrent / WebTorrent / magnet
- IPFS / IPNS
- Hypercore / `hyper://`
- NNTP
- XMPP pubsub
- NNCP
- I2P / Tor

Most of these should begin as **receivable or integrative lanes**, not as full native stacks.

---

## 7. Highest-Leverage Gaps Identified Across The Discussion

These are the most valuable additions or clarifications surfaced by the combined inputs.

### 7.1 WebFinger should be first-class in the identity/discovery model

Current docs mention WebFinger, but it is still somewhat secondary in wording.

The synthesis view is:

- WebFinger is not just "preferred over Finger."
- It is a practical human-handle entry point into the social/smallnet identity story.
- A lookup like `alice@example.com` can become the pivot through which Graphshell discovers:
  - Gemini capsule URL,
  - Misfin address,
  - Nostr identity hints,
  - ActivityPub actor,
  - other domain-bound identity endpoints.

This deserves first-class treatment in protocol/discovery docs.

### 7.2 NIP-05 + WebFinger + person-node convergence is missing

This is one of the strongest insights from the final gap analysis.

Graphshell needs a "person node" model capable of resolving and storing:

- human handle (`alice@example.com`),
- Nostr `npub`,
- NIP-05 proof,
- Gemini capsule endpoint,
- Misfin endpoint,
- Matrix MXID,
- ActivityPub actor URL,
- additional verified endpoints.

This would turn the social layer from a bag of protocols into a coherent graph-native identity model.

### 7.3 WebRTC data-channel fallback for browser envelopes

This is likely the most urgent practical transport gap.

The current co-op path is iroh-first, which is correct for native. But browser/PWA/extension envelopes cannot rely on native QUIC behavior being equivalently available.

Therefore:

- co-op in browser envelopes likely needs an explicit WebRTC data-channel fallback path,
- or co-op in those envelopes must be honestly documented as degraded/unavailable.

This is important because co-op is central to the pitch, and extension/PWA envelopes are strategically important in the host-envelope model.

### 7.4 A unified Graphshell/Verse URI scheme needs a real spec

Multiple docs already imply internal/external Graphshell-addressable URIs:

- cabal invite URIs,
- co-op session invites,
- graph node sharing,
- community addresses,
- graph snapshot references.

These should stop accumulating ad hoc.

A single URI scheme spec should cover at least:

- node address,
- graph/workspace/snapshot address,
- co-op invite address,
- community/room/cabal address,
- published artifact address.

### 7.5 IPFS/IPNS and BitTorrent are relevant as receivable formats

These should not necessarily become core architectural commitments, but they matter as practical content lanes.

- `ipfs://` and `ipns://` fit naturally with Verse's content-addressing story.
- `magnet:` and torrent receipt support would strengthen the archival/distributed-file story and connect to a different real peer network than libp2p.

This is less about ideological completeness than about making Graphshell capable of receiving the kinds of artifacts its users are likely to encounter.

### 7.6 JSON Feed should sit beside RSS/Atom

This is a simple but worthwhile completion.

If Graphshell claims a feed/document lane, then:

- RSS,
- Atom,
- JSON Feed

should be treated as one cluster.

### 7.7 ActivityPub should at least exist as a read-only lane

Graphshell already cares about structured web, social publication, and community knowledge.

That makes ActivityPub relevant even if full federation is not a priority.

The most realistic first move is:

- read-only ActivityPub ingestion/parsing,
- ActivityStreams JSON-LD interpretation,
- graph representation of actors, posts, replies, and linked artifacts.

This opens a path into the fediverse without requiring Graphshell to become a fediverse server.

---

## 8. Useful But Not Yet Validated Ideas

Some supplied ideas are worth preserving, but they should remain clearly marked as unvalidated until anchored by stronger evidence or design need.

### 8.1 Sans-I/O protocol layering

This is a strong architectural suggestion.

- Smallnet protocols should be implemented in a transport-agnostic way where possible.
- Native hosts can provide raw sockets directly.
- Browser-class hosts can route through permitted transports or proxy layers.

This is especially relevant because raw TCP/UDP is not available in ordinary browser WASM environments.

### 8.2 OPFS for browser-host persistence

Worth tracking as a browser-host storage substrate, especially for:

- graph snapshots,
- indexes,
- cached content,
- local-first metadata.

This should be evaluated against the already-adopted browser-storage authority model rather than bolted on ad hoc.

### 8.3 Scorpion as "small HTTP"

Scorpion is technically interesting because it compresses several desired semantics into one protocol family:

- richer methods,
- uploads,
- range requests,
- optional TLS.

However, community traction appears low and there is no need to promote it beyond "interesting future candidate" right now.

### 8.4 Text Protocol and Mercury

These may be worth noting as ecosystem awareness items, but not as active roadmap candidates without stronger product evidence.

### 8.5 Willow and other sync/data models

Ideas like Willow may still be strategically relevant as comparative research, especially for hierarchical naming and sync, but should not silently displace the current iroh/libp2p model without a dedicated architecture pass.

---

## 9. Recommendations By Priority

### 9.1 High priority

1. Write a first-class identity convergence note:
   - WebFinger,
   - NIP-05,
   - person-node model,
   - endpoint binding rules.

2. Write a Graphshell/Verse URI scheme spec:
   - nodes,
   - sessions,
   - rooms/cabals/communities,
   - portable graph artifacts.

3. Decide and document the browser-envelope co-op reality:
   - WebRTC fallback,
   - degraded support,
   - or explicit non-support for now.

4. Expand protocol/discovery docs so WebFinger is listed as a first-class discovery lane, not only as a preference note.

### 9.2 Medium priority

1. Add JSON Feed to the feed/document protocol cluster.
2. Add `ipfs://` / `ipns://` as explicit receivable URL-scheme candidates.
3. Add a read-only ActivityPub/ActivityStreams research note.
4. Add BitTorrent/WebTorrent/magnet as receivable protocol research.
5. Add an explicit graph-object classification model for publication vs annotation vs traversal vs ephemeral session state.

### 9.3 Low priority

1. WebSub
2. WebMention
3. IndieAuth / RelMeAuth
4. Scorpion
5. XMPP pubsub
6. NNCP / sneakernet lanes
7. Mercury / Text Protocol / Scroll / other niche smallnet variants

---

## 10. Organizing Protocols By User Job

One recurring problem in protocol research is that the protocol list becomes
larger than the product model.

Graphshell should not primarily organize protocols as:

- old internet protocols,
- smallnet protocols,
- federated protocols,
- P2P protocols,
- storage protocols.

That organization is useful for research, but not for product design.

For product and architecture planning, protocols are more useful when grouped by
the **job the user is trying to do**.

### 10.1 Open and read a thing

These are the protocols and formats the user directly experiences as readable
documents or directories:

- Gemini
- gemtext
- Gopher
- Finger
- Spartan
- Nex
- static HTML
- Markdown
- plain text
- RSS
- Atom
- JSON Feed
- gempub

This is the strongest candidate for the practical "middlenet" surface.

### 10.2 Find a person, publication, or endpoint

These protocols help Graphshell turn a human handle or domain into reachable
services and identities:

- WebFinger
- NIP-05
- DNS-SD / local discovery where relevant
- `.well-known` discovery lanes

This cluster is especially important because it turns "who is this?" into
"where can I reach them and what can they publish?"

### 10.3 Publish, send, or update something

These are mutation or outward-publication lanes:

- Titan
- Misfin
- WebMention
- ActivityPub write path, if ever adopted

This cluster should stay distinct from passive browsing because the UX,
permissions, and auditability needs are different.

### 10.4 Follow, subscribe, and discover

These protocols or formats support updates, public subscriptions, and
trust-mediated discovery:

- RSS / Atom / JSON Feed
- WebSub
- Nostr
- ActivityPub read path

This is a different user job from "open this URL once." It is about durable
attention and discovery flow.

### 10.5 Browse together or stay in touch

These support collaboration, room presence, session continuity, and social
contact:

- iroh
- WebRTC
- Matrix
- Cable
- Nostr DMs / relay messaging where appropriate

These are not document protocols, even though they may appear alongside
document protocols in user flows.

### 10.6 Keep this available, sync it, or fetch it from peers

These are storage, replication, or receivable distribution lanes:

- VerseBlob
- IPFS / IPNS
- BitTorrent / WebTorrent / magnet
- Hypercore
- NNCP

This cluster is where "append-only logs," "content addressing," and distributed
replication belong. Hypercore is best thought of here, not as a browsing
protocol.

### 10.7 Sign in, prove identity, or bind personas

These are identity-binding and verification lanes:

- Nostr identities / NIP-05
- WebFinger
- Matrix identity bindings
- ActivityPub actor identities
- IndieAuth / RelMeAuth, if ever added

This is a separate concern from transport and separate again from browsing.

### 10.8 Useful implementation rule

If a protocol primarily answers:

- "what do I render?" then it is a **document lane**
- "who is this?" then it is an **identity/discovery lane**
- "how do I publish or send?" then it is a **mutation lane**
- "how do we stay present together?" then it is a **collaboration lane**
- "where does the data live and how does it replicate?" then it is a
  **storage/replication lane**

This rule prevents Graphshell from treating every protocol as if it belongs in
the same registry or product surface.

### 10.9 User-facing surface emphasis

The protocol landscape becomes easier to manage when Graphshell uses a small
set of user-facing product surfaces first, and only derives tier shorthand
from those surfaces afterward.

#### Document surface

These are the protocols and formats with the strongest claim on first-class
reading, rendering, and authored-document UI support:

- Gemini
- gemtext
- Gopher
- Finger
- Spartan
- Nex
- RSS / Atom / JSON Feed
- static HTML
- Markdown / plain text
- gempub

#### Identity and discovery surface

These resolve people, handles, and service endpoints into graph-native identity
objects rather than rendered documents:

- WebFinger
- NIP-05
- Matrix identity bindings
- ActivityPub actor identities

#### Mutation and messaging surface

These protocols need explicit permissions, auditability, and outcome handling
rather than being treated as passive browsing:

- Titan
- Misfin
- WebMention
- ActivityPub write path, if ever adopted

#### Collaboration and presence surface

These matter a lot, but they support a bigger collaborative feature rather than
the first visible browsing surface:

- Matrix
- WebRTC fallback
- iroh
- Cable
- Nostr DMs / relay messaging where appropriate

#### Storage and receivable artifact surface

These provide fetch, sync, replication, or receivable artifact transport:

- VerseBlob
- IPFS / IPNS
- BitTorrent / WebTorrent / magnet
- Hypercore
- NNCP

#### Research and niche lanes

These may still be strategically useful, but should not compete for immediate
product attention unless a very specific use case emerges:

- Scorpion
- Mercury
- Text Protocol
- Scroll
- XMPP pubsub
- other niche smallnet variants

If Graphshell keeps a Tier A/B/C shorthand, it should be attached to specific
capabilities within a surface, such as a first-class document lane or an
experimental storage lane, rather than used as a single mixed ranking across
unlike protocol jobs.

### 10.10 Why this matters

Without this re-organization, the project risks asking the wrong question:

- "which protocols should we support?"

The better question is:

- "which user jobs should Graphshell make excellent, and which protocols serve
  those jobs?"

That keeps the protocol landscape useful instead of overwhelming.

### 10.11 Follow-on implementation rule: protocol capabilities first

Once Graphshell supports more than a handful of protocols, per-protocol routing
logic starts to leak everywhere:

- action dispatch,
- trust handling,
- identity import,
- publication,
- message delivery,
- and endpoint selection from person nodes.

The next architectural step should be a **protocol capability model** that
answers questions like:

- can this protocol discover an identity?
- can it resolve a profile?
- can it publish an artifact?
- can it deliver a message?
- does it depend on HTTP fetch or a known-hosts trust store?

This is more important than it sounds. Without a capability layer, Graphshell
keeps asking whether a flow is WebFinger, Matrix, ActivityPub, Titan, or
Misfin. With a capability layer, Graphshell can instead ask what the user is
trying to do and then choose the protocol(s) that satisfy that job.

That shift matters for four reasons:

- it reduces stringly-typed protocol branching in app code,
- it gives future protocols a stable slot in the architecture,
- it makes trust/caching policy easier to share across protocols,
- and it keeps the product model organized by user job rather than by protocol
  trivia.

The first practical slice should stay narrow:

- define a small descriptor table for the current Middlenet protocols,
- record identity/discovery, mutation, and trust-related capabilities,
- route identity-import normalization through that table,
- and use capability-based endpoint selection when person nodes publish or
  deliver through protocol-specific lanes.

That is enough to establish the abstraction without pretending the whole
protocol stack has already been generalized.

The next slice after that should also stay concrete:

- cache resolved identity profiles by `(protocol, normalized query)`,
- record the normalized query as durable person-node provenance,
- and append audit history that captures cache hit/miss state, source
  endpoint(s), and resolution time.

That gives Graphshell a usable notion of identity freshness and provenance
before it attempts a larger cross-protocol refresh scheduler or trust-policy
unification.

---

## 11. Suggested Near-Term Product Framing

To avoid overselling and reduce scope pressure, the project can be framed more cleanly like this:

Graphshell is building:

- a **graph-first browser and knowledge workspace**,
- a **portable middlenet/smallnet document engine**,
- a **faithful small-protocol browser with strong identity and collaboration ambitions**,
- and a **layered path** toward broader structured-web and community-network support.

It is not yet accurate to frame the project as "basically a full browser, just waiting on the JIT."

A more honest and strategically stronger formulation is:

- first win Tier 0/1 and graph-native browsing,
- then win reader/document-grade middlenet,
- then expand host/runtime capability where WebRender and browser-runtime work make it justified.

---

## 12. Durable Conclusions

The combined discussion supports these durable conclusions:

- The graph-first browser thesis is coherent and already well-supported by current architecture.
- The host-envelope model is sound and should remain foundational.
- The biggest missing pieces are mostly about identity, addressability, trust UX, and browser-envelope transport realities.
- The most important near-term protocol completions are not obscure smallnet variants; they are WebFinger, identity convergence, JSON Feed, explicit receivable distributed-content lanes, and a real URI/addressability spec.
- The most urgent practical feature gap is co-op transport in browser-class hosts.
- The middlenet vision remains strategically good, but near-term product truth is constrained by the WebRender/wgpu path and should be framed accordingly.

---

## 13. Non-Conclusions

This document does **not** conclude that Graphshell should:

- implement every listed smallnet or alternative-web protocol,
- replace its current iroh/libp2p/Nostr/Matrix layer split,
- commit to Scorpion, Mercury, Text Protocol, or similar niche protocols,
- treat all speculative ideas as roadmap obligations,
- collapse Verse and Verso into a single undifferentiated network layer.

---

## 14. Provenance Of Inputs

This synthesis consolidates:

- the project-owner middlenet/graph-browser overview,
- an initial critique emphasizing boundaries over feature accretion,
- a broad "small internet protocol comprehensive research" inventory,
- a second architecture response emphasizing transport constraints in WASM/browser hosts,
- a final gap analysis emphasizing identity convergence, URI addressability, receivable distributed-content lanes, and browser-envelope co-op transport.

It intentionally preserves useful insights from weaker or more naive inputs without upgrading them to canonical truth.
