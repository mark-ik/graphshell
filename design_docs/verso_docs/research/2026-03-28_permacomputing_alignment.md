<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Permacomputing Alignment Research

**Date**: 2026-03-28
**Status**: Research / Design Exploration
**Purpose**: Evaluate how Graphshell and Verso align with permacomputing principles, identify gaps, and propose actionable design directions drawn from the permacomputing project ecosystem.

**Sources**:

- [permacomputing.net](https://permacomputing.net/) — wiki and community hub
- [permacomputing.net/projects/](https://permacomputing.net/projects/) — curated project index
- [permacomputing.net/principles/](https://permacomputing.net/Principles/) — ten design principles

**Related**:

- [`../technical_architecture/VERSO_AS_PEER.md`](../technical_architecture/VERSO_AS_PEER.md) — Verso mod boundary and protocol server specs
- [`../../graphshell_docs/research/2026-02-27_freenet_takeaways_for_graphshell.md`](../../graphshell_docs/research/2026-02-27_freenet_takeaways_for_graphshell.md) — prior P2P ecosystem research
- [`../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md`](../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md) — Verse community-scale network
- [`../implementation_strategy/2026-03-28_gemini_capsule_server_plan.md`](../implementation_strategy/2026-03-28_gemini_capsule_server_plan.md) — Gemini/Gopher/Finger capsule servers

---

## 1. What Permacomputing Is

Permacomputing is a philosophy and community of practice oriented around resilience and regenerativity in computing, directly inspired by permaculture. It rests on three permaculture ethics extended into the digital domain: **Earth Care**, **People Care**, and **Fair Share**.

The movement challenges extractive, growth-obsessed patterns in modern computing: planned obsolescence, software bloat, vendor lock-in, infinite data accumulation, and the environmental costs of semiconductor manufacturing and cloud infrastructure.

There is "no permacomputing kit to buy." It is a collective rethinking of computational culture, not a product line.

---

## 2. The Ten Principles (Summarized)

| # | Principle | Core idea |
|---|-----------|-----------|
| 1 | Hope for the Best, Prepare for the Worst | Design resilient systems tolerant to interruptions and collapse |
| 2 | Care for All Hardware | Maximize lifespan of existing hardware, especially chips |
| 3 | Observe First | Understand context before building; question whether tech is needed |
| 4 | Not Doing (Refusal) | Embrace degrowth; curb demand through refusal |
| 5 | Expose the Seams | Reject seamlessness that hides infrastructure and costs |
| 6 | Simplicity, Complexity, Scale | No magic bullet; match solution complexity to problem complexity |
| 7 | Keep It Flexible | Adapt to changing environments; 24/7 availability is not required |
| 8 | Build on Solid Ground | Minimize obsolescence via mature tech, open standards, documented formats |
| 9 | (Almost) Everything Has a Place | Nothing is inherently obsolete; cultivate diverse computing cultures |
| 10 | Integrate Biological and Renewable Resources | Work with local, renewable materials where possible |

---

## 3. Where Graphshell Already Aligns

### 3.1 Build on Solid Ground (Principle 8)

Rust as the implementation language: no garbage collector, zero-cost abstractions, strong safety guarantees, compiles to native code. The dependency on Servo (Mozilla lineage, open-source, well-documented rendering engine) and egui (immediate-mode, minimal, well-understood) reflects a preference for mature, inspectable foundations.

Data formats use open standards: fjall/redb for local storage, rkyv for wire serialization, Ed25519 for identity. No proprietary protocols.

### 3.2 Expose the Seams (Principle 5)

A spatial browser that renders navigation topology as a force-directed graph is a direct embodiment of this principle. Where conventional browsers hide the structure of browsing behind a linear tab strip or history list, Graphshell makes the topology visible, inspectable, and manipulable. Users see where they've been, how pages relate, and what clusters of activity exist.

The diagnostic channel system (`ChannelSeverity`, `DiagnosticChannelDescriptor`) makes internal system state observable by design.

### 3.3 Keep It Flexible (Principle 7)

Verso is an optional mod. Graphshell without Verso is a visual outliner/file manager — all core viewers work, no web access required. This is the Unix "small, sharp tools" philosophy applied to a browser: the shell is useful on its own; networking is additive.

The registry/mod architecture (`ViewerRegistry`, `ProtocolRegistry`, `ActionRegistry`) allows capability to be composed at startup rather than hardcoded.

### 3.4 (Almost) Everything Has a Place (Principle 9)

Gemini, Gopher, and Finger protocol servers already exist in Verso. These protocols are decades old (Finger: 1977, Gopher: 1991) and are treated as living capability lanes, not historical curiosities. The `SimpleDocument` model provides a common content representation that all three protocols can serialize to and from.

### 3.5 Hope for the Best, Prepare for the Worst (Principle 1)

Local-first architecture: the semantic graph lives locally in fjall, layout is device-local, sync is optional and bilateral. Network failure degrades gracefully — the user always has their graph. Device Sync version vectors tolerate intermittent connectivity.

---

## 4. Gaps and Opportunities

### 4.1 Resource Awareness Surface — "Observe First" (Principle 3)

**Gap**: Graphshell does not currently expose per-node or per-session resource costs (bandwidth consumed, storage used, energy proxy) to the user.

**Opportunity**: The bilateral `PeerStorageReport` in VERSO_AS_PEER.md (§Bilateral Storage Visibility) already tracks bytes held per peer. Extend this pattern to browsing:

- Per-node metadata: bytes fetched, cache size, request count, last-fetch timestamp.
- Session-level aggregate: total bandwidth for a browsing session.
- Graph-canvas visualization: node size, color temperature, or badge reflecting resource weight.

This makes the cost of browsing tangible — users can see which nodes are heavy and make informed decisions. Aligns with Watt Wiser (energy visualization) and Solar Protocol (energy-aware routing) from the permacomputing project index.

**Scope**: Diagnostic channel or graph metadata extension. Not a core architectural change.

### 4.2 Intentional Forgetting / Decay — "Not Doing" (Principle 4)

**Gap**: The graph model accumulates nodes and edges indefinitely. There is no built-in decay, expiry, or intentional forgetting policy.

**Opportunity**: Introduce optional decay semantics:

- Time-based soft expiry: nodes not visited or referenced within a configurable window fade visually and can be bulk-archived.
- Session-scoped ephemerality: nodes created during a co-op session can be marked ephemeral-by-default, automatically pruned after session end unless explicitly promoted.
- Archive privacy classes already exist (`LocalPrivate`, `OwnDevicesOnly`, `TrustedPeers`, `PublicPortable`). An `Ephemeral` class or a TTL field would extend this naturally.

This counters append-only maximalism. Cable's `post/delete` (see §5 below) demonstrates that peer-to-peer protocols can support actual deletion without architectural compromise.

**Scope**: Graph model extension (TTL field or decay policy on `NodeMetadata`). UX: visual fade + bulk archive action.

### 4.3 Constrained Hardware Target — "Care for All Hardware" (Principle 2)

**Gap**: Graphshell's current rendering target assumes a GPU-capable desktop. There is no defined behavior for constrained environments (low RAM, no GPU, small screen).

**Opportunity**: Define a "lean mode" profile:

- Text-mode graph outline (tree view fallback when GPU canvas is unavailable or too expensive).
- Disable Servo entirely; use only core viewers (`PlaintextViewer`, `ImageViewer`, `PdfViewer`).
- Reduce physics simulation budget (fewer iterations, coarser force model).

This does not require a separate binary — the registry/mod architecture already supports it. A `--lean` flag or runtime detection of hardware constraints could select the appropriate viewer and canvas policy set.

**Scope**: Canvas policy extension + optional text-mode outline renderer. Medium effort.

### 4.4 Gemini-First Publishing UX — "Build on Solid Ground" (Principle 8)

**Current state**: The Gemini capsule server is implemented. `ServeNodeAsGemini` is a `GraphIntent`. Content routes exist.

**Opportunity**: Make "publish to Gemini" a prominent, one-click action in the workbench toolbar or node context menu. Frame it as "share this on the small web." This positions Graphshell not just as a consumer of the web but as a tool for *producing* resilient, lightweight web presence — a core permacomputing value.

The same applies to Gopher and Finger, but Gemini is the primary modern target.

**Scope**: UX/toolbar integration. The backend is already in place.

### 4.5 Content Portability — "Build on Solid Ground" (Principle 8)

**Inspiration**: Uxn's ROM model — self-contained ~15kb applications with no external dependencies that can run on any platform with a Uxn VM.

**Opportunity**: Define a "maximally portable graph node" export format. A `SimpleDocument` + metadata bundle that any future tool can parse without needing Graphshell. This is partially addressed by `SessionCapsule` (CID-addressed, encrypted, WASM-safe), but the emphasis should be on *openness*: a node exported as a standalone artifact should be readable by any tool that understands a simple, documented format (e.g., a directory containing a Gemtext file + a JSON metadata sidecar).

**Scope**: Export format definition. Builds on existing `SimpleDocument` and `SessionCapsule` work.

---

## 5. Relevant Projects from the Permacomputing Index

| Project | What it is | Relevance to Graphshell |
|---------|-----------|------------------------|
| **Gemini Protocol** | Lightweight application-layer protocol (2019). Mandatory TLS, simple hyperlinks, text/gemini format. | Already integrated — capsule server in Verso. |
| **Cable** | Decentralized P2P group chat protocol. Binary wire format, Ed25519 identity, subjective moderation, pull-based sync. Rust implementation exists. | Strong candidate for co-op minichat substrate. See separate spec. |
| **Uxn** | Minimal VM by 100 Rabbits. 32 opcodes, ~100 lines of C, ~15kb ROMs. | Reference model for maximally portable content. |
| **100 Rabbits** | Artist collective. Offline-first, solar-powered, minimal creative toolchain. | Philosophy reference for tool design under constraints. |
| **Coalescent Computer** | "Simple and unenclosable social computing platform designed to replace the architecture and use cases of the World Wide Web." | Same design space as Graphshell. Worth deeper investigation. |
| **Solar Protocol** | Distributed web platform routing traffic to solar-powered servers. | Energy-aware routing model. Relevant to Verse relay selection. |
| **Watt Wiser** | Desktop energy measurement and visualization tools. | Reference for §4.1 resource awareness surface. |
| **snac** | Minimalistic ActivityPub instance in portable C. | Demonstrates that federated social protocols can be radically simple. Relevant to Comms applet design. |
| **Collapse OS / Civboot** | Systems designed to preserve computing capability through collapse. FORTH-based (Collapse OS) or self-bootstrapping (Civboot). | Extreme "Build on Solid Ground" — informs thinking about software longevity. |
| **Mu** | Minimal-dependency computing stack from machine code. 2/3 of code is automated tests. | Demonstrates that comprehensibility and type safety coexist with radical minimalism. |
| **Cerca** | Lean web forum software. | Reference for minimalist community surfaces (Comms applet). |

---

## 6. Design Posture Summary

Graphshell's existing architecture is permacomputing-compatible by accident of good design instincts: local-first, modular, protocol-diverse, Rust-native. The gaps are mostly in **user-facing expression** of these values:

- Make resource costs visible (§4.1).
- Support intentional forgetting (§4.2).
- Degrade gracefully to constrained hardware (§4.3).
- Surface small-web publishing as a primary action (§4.4).
- Define maximally portable export (§4.5).

None of these require architectural rewrites. They are extensions of existing patterns — diagnostic channels, canvas policies, registry capabilities, and export formats.

The strongest actionable connection is Cable as a co-op minichat protocol (see companion spec: `../implementation_strategy/2026-03-28_cable_coop_minichat_spec.md`).
