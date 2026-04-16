<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Middlenet Lane Architecture Spec

**Date**: 2026-04-16
**Status**: Proposed architectural spec
**Scope**: Define the crate split, lane model, selection contract, and shared
host plumbing for a protocol-first Middlenet engine that can choose between a
direct portable document renderer, a richer static HTML/CSS renderer, and full
web fallback.

**Related docs**:

- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
  — baseline Middlenet scope, portable-engine framing, and intermediate
  document model direction
- [`2026-03-30_protocol_modularity_and_host_capability_model.md`](2026-03-30_protocol_modularity_and_host_capability_model.md)
  — host-aware degradation and protocol packaging classes
- [`2026-02-18_universal_node_content_model.md`](2026-02-18_universal_node_content_model.md)
  — node as persistent content container independent of renderer
- [`../implementation_strategy/viewer/universal_content_model_spec.md`](../implementation_strategy/viewer/universal_content_model_spec.md)
  — viewer routing and content selection policy
- [`../implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md`](../implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md)
  — fallback/degraded-state expectations
- [`../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`](../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md)
  — vello/parley/AccessKit direction and custom-canvas strategy
- [`../research/2026-04-14_wasm_portable_renderer_feasibility.md`](../research/2026-04-14_wasm_portable_renderer_feasibility.md)
  — portable renderer feasibility and host/WASM constraints

---

## 1. Problem Statement

Graphshell's Middlenet goal is broader than "lightweight HTML rendering."
MiddleNet covers protocol-faithful documents across Gemini, Gopher, Finger,
feeds, Markdown, static HTML, article-mode HTTP, observation cards, previews,
and archived/offline content.

The architecture therefore must not collapse around one renderer's internal DOM.

The design problem is:

- preserve a protocol-first document model,
- support a very portable direct render lane,
- allow a richer static HTML/CSS lane where it materially helps,
- keep full web fallback available,
- avoid forcing every surface to pay the cost of the heaviest engine.

This spec names that solution the **lane architecture**.

---

## 2. Core Invariants

### 2.1 Semantic document truth is canonical

Graphshell MUST preserve a renderer-independent canonical Middlenet document
representation.

That canonical representation:

- MUST be suitable for protocol-faithful rendering,
- MUST carry provenance and source metadata,
- MUST be portable across native and WASM hosts,
- MUST remain valid even if a renderer crate is replaced.

Blitz DOM, Servo DOM, and any renderer-specific tree are **derived working
representations**, not canonical truth.

### 2.2 Lanes are replaceable execution strategies

A lane is a rendering/execution strategy selected for a request. Lanes MUST be
modular and host-aware. No single lane may define Middlenet as a whole.

### 2.3 Host plumbing is shared

Font resolution, image decode/cache, accessibility projection, offscreen
rendering, hit-testing metadata, and texture/export surfaces SHOULD be shared
across lanes where practical. Renderer-specific internals MAY differ, but the
host-facing contracts SHOULD converge.

### 2.4 Servo remains a complement, not a failure

Selecting the Servo lane for full-web or JS-heavy content is an intended part of
the architecture, not an escape hatch that invalidates Middlenet.

---

## 3. Lane Model

Graphshell SHOULD support at least four lanes.

### 3.1 Direct Lane

The **Direct Lane** renders canonical Middlenet documents without treating HTML
as the center of the universe.

Primary use cases:

- hover previews
- Verse search results
- observation cards
- Gemini/Gopher/Finger rendering
- feeds and Markdown
- offline/archived document viewing
- highly portable WASM targets

Typical stack:

- `middlenet-core` semantic document
- `middlenet-render`
- `parley`
- `vello`
- optional `anyrender` backend abstraction

### 3.2 Blitz Lane

The **Blitz Lane** renders static-ish HTML/CSS content through a richer
HTML/CSS engine when that yields materially better fidelity than the Direct
Lane.

Primary use cases:

- article-mode HTTP
- captured HTML clips
- archived static pages
- richer local publication/document surfaces
- static observation-card details that benefit from CSS layout

The Blitz Lane MUST remain optional. It is a Middlenet lane, not the definition
of Middlenet itself.

### 3.3 Servo Lane

The **Servo Lane** handles content that needs a full browser engine.

Primary use cases:

- live modern web pages
- authenticated surfaces
- JS-heavy or app-like sites
- layout/API cases beyond Middlenet's intended scope

### 3.4 Raw Source Lane

The **Raw Source Lane** preserves faithful-source fallback where structural
adaptation is misleading, unsupported, or user-disabled.

Primary use cases:

- raw gophermap
- exact gemtext/source view
- parse failures
- diagnostics and archival fidelity

---

## 4. Crate Responsibilities

The lane architecture should be expressed as a crate split.

| Crate | Responsibility | Canonical? |
|---|---|---|
| `middlenet-core` | Semantic document model, provenance, metadata, observation-card view model, render request/response types | Yes |
| `middlenet-adapters` | Gemini/Gopher/Finger/Markdown/feed/plain-text/article adapters into `middlenet-core` | Yes |
| `middlenet-render` | Direct Lane renderer for canonical Middlenet documents | No |
| `middlenet-blitz` | Blitz-backed HTML/CSS lane adapter | No |
| `middlenet-servo` | Servo-backed lane adapter and capability bridge | No |
| `middlenet-engine` | Facade/orchestrator: source detection, lane scoring, selection, fallback, host integration | No |

### 4.1 `middlenet-core`

`middlenet-core` SHOULD own:

- canonical semantic document tree
- canonical source/provenance metadata
- link/action model
- title/snippet extraction
- observation-card rendering payloads
- lane request/response contracts
- lane override enums
- host-capability-neutral display model

It MUST NOT depend on Servo, Blitz, egui, or host-native viewers.

### 4.2 `middlenet-adapters`

`middlenet-adapters` SHOULD own:

- protocol parsing for Gemini/Gopher/Finger
- feed parsing
- Markdown/plain-text parsing
- source detection
- article/readability extraction
- HTML import into canonical semantic structures where appropriate

Adapters MUST output `middlenet-core` data.

### 4.3 `middlenet-render`

`middlenet-render` SHOULD own:

- Direct Lane scene derivation
- text layout
- paint model
- link geometry and hit-testing metadata
- offscreen thumbnail/preview generation
- optional CPU/GPU backend choice

`anyrender` MAY be used here as a backend abstraction layer. If adopted, it is
host/render plumbing, not canonical truth.

### 4.4 `middlenet-blitz`

`middlenet-blitz` SHOULD own:

- sanitized/static HTML ingestion into Blitz
- Blitz-specific style/layout/paint setup
- bridging canonical Middlenet metadata into Blitz-rendered documents
- extracting hit-test/link/selectable metadata back into Graphshell contracts

It MUST NOT redefine Middlenet's canonical document model.

### 4.5 `middlenet-servo`

`middlenet-servo` SHOULD own:

- Servo delegation for complex content
- lane-specific capability probes
- rendering handoff contracts for node viewers/panes
- policy checks for when a request must escalate to full browser behavior

### 4.6 `middlenet-engine`

`middlenet-engine` SHOULD remain the facade visible to the rest of Graphshell.

It SHOULD expose:

- source detection
- lane scoring and selection
- explicit lane override API
- shared fallback behavior
- host-capability routing

---

## 5. Canonical Types

The exact final data shapes may evolve, but the architecture assumes the
following carrier classes.

### 5.1 `SemanticDocument`

Canonical Middlenet document truth.

Must represent:

- headings
- paragraphs
- links/actions
- lists
- code/preformatted blocks
- quotes
- separators/sections
- metadata slots for title, summary, timestamps, provenance

It MAY later widen to richer inline formatting and embedded media descriptors,
but it should start from protocol-faithful document primitives, not browser DOM
maximalism.

### 5.2 `RenderRequest`

A lane-selection input carrier.

Should include:

- canonical source kind
- URI and provenance
- whether content is live, captured, archived, or generated
- whether JS/forms/auth are required
- fidelity preference
- offline requirement
- preview/detail mode
- host target
- explicit user lane override

### 5.3 `HostCapabilities`

Describes what the current host can do.

Should include:

- native vs WASM vs WASI
- GPU/CPU rendering availability
- offscreen image export availability
- accessibility surface availability
- embedded web-engine availability
- network policy / offline mode

### 5.4 `RenderOutput`

Host-facing lane output.

Should include:

- display list, scene, or texture output
- hit-test regions
- link/action map
- accessibility projection payload
- diagnostics payload
- fallback explanation if degraded

---

## 6. Lane Selection Contract

Lane choice MUST be deterministic, explainable, and overridable.

### 6.1 Selection phases

1. Detect source/content kind.
2. Build canonical `RenderRequest`.
3. Score available lanes against request + host capabilities.
4. Apply explicit user override if valid.
5. Choose best lane.
6. If chosen lane fails, walk fallback chain with recorded reason.

### 6.2 Baseline precedence

The default preference order SHOULD be:

- Direct Lane for canonical Middlenet documents and preview/archive surfaces
- Blitz Lane for static HTML/CSS or richer article/captured surfaces
- Servo Lane for full-web requirements
- Raw Source Lane when fidelity or parse conditions require it

Exact scoring may vary, but the user-visible result MUST be explainable in terms
of content needs and host capability.

### 6.3 Override model

Each node/document SHOULD support:

- `Auto`
- `Direct`
- `Blitz`
- `Servo`
- `Raw`

If an override is impossible on the current host, Graphshell MUST explain why.

### 6.4 Example routing rules

- Gemini/Gopher/Finger → Direct by default, Raw optional
- Markdown/feed/plain text → Direct by default
- captured static HTML/article mode → Blitz preferred, Direct acceptable fallback
- observation card / search result card / hover preview → Direct preferred
- JS-required or authenticated page → Servo

---

## 7. Shared Host Plumbing

The following concerns SHOULD be host-shared rather than lane-owned wherever
possible:

- font registry and fallback
- image decode/cache
- texture allocation or surface handoff
- offscreen render-to-image
- link/action hit-testing contract
- selection/hover metadata contract
- accessibility tree projection
- diagnostics and render-failure reporting
- thumbnail and preview cache

This is where `anyrender` can add value:

- one scene API
- GPU or CPU rendering backends
- live rendering and image-buffer rendering from the same abstraction

If used, `anyrender` should sit below lane contracts, not above canonical
document truth.

---

## 8. Current `middlenet-engine` Mapping

The current crate already contains the seeds of the lane split.

| Current file | Future home |
|---|---|
| `document.rs` | `middlenet-core` |
| `source.rs` | `middlenet-core` / `middlenet-adapters` |
| `adapters.rs` | `middlenet-adapters` |
| `engine.rs` | `middlenet-engine` facade |
| `viewer.rs` | `middlenet-render` host-facing direct renderer contract |
| `dom.rs`, `style.rs`, `layout.rs`, `compositor.rs`, `script.rs` | split between `middlenet-render` and `middlenet-blitz`, rather than remaining one undifferentiated "mini browser" blob |

### 8.1 Architectural correction

The main correction this spec makes is:

- do not grow the current DOM/style/layout/script scaffolding into one monolithic
  Middlenet mini-browser,
- instead split it into lane-specific modules behind a canonical semantic
  document core.

That preserves the Middlenet dream while avoiding a misleading "HTML engine
first" center of gravity.

---

## 9. Surface Mapping

The lane architecture should map to Graphshell surfaces as follows.

| Surface | Preferred lane | Notes |
|---|---|---|
| Hover preview | Direct | Fast, deterministic, offscreen-friendly |
| Verse search result card | Direct | Observation-card and snippet-native |
| Observation-card detail | Direct, optional Blitz | Blitz only if richer local HTML styling adds value |
| Offline/article archive | Blitz, fallback Direct | Static HTML/CSS lane is attractive here |
| Feed reader / Markdown viewer | Direct | Protocol/document-first |
| Live website pane | Servo | Full browser engine required |
| Raw protocol/source inspector | Raw | Fidelity/debug view |

This "pick your lane" model is a feature, not an implementation accident.

---

## 10. Non-Goals

This spec does not require:

- replacing Servo
- forcing all content through Blitz
- forcing HTML as Middlenet's canonical document model
- immediate support for full browser interactivity in portable/WASM lanes
- locking the project to any single rendering backend abstraction

---

## 11. Recommended Execution Order

1. Stabilize and widen `SimpleDocument` into a canonical `SemanticDocument`.
2. Extract `middlenet-core` and `middlenet-adapters`.
3. Build the Direct Lane (`middlenet-render`) first.
4. Route hover previews, search results, and observation cards through Direct.
5. Add explicit lane chooser API in `middlenet-engine`.
6. Add `middlenet-blitz` as an optional richer static HTML lane.
7. Keep Servo as the full-web lane and narrow delegation only when Direct/Blitz
   are truly ready.

This ordering favors the most portable, deterministic, and broadly useful lane
first.

---

## 12. Design Summary

The Middlenet lane architecture turns Graphshell into a **modular browser** in a
concrete, disciplined sense:

- protocol-first core,
- multiple renderer/execution lanes,
- shared host plumbing,
- explicit lane selection,
- no single engine monopolizing the definition of content truth.

That is a better fit for Graphshell than either:

- making Blitz the center of Middlenet, or
- growing Middlenet into a monolithic mini browser engine.
