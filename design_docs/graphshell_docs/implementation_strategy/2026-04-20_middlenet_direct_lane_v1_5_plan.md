<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Middlenet Direct Lane v1.5 Plan

**Date**: 2026-04-20
**Status**: Proposed milestone plan (Medium scope)
**Owner**: Middlenet
**Scope**: Turn the Direct Lane from "semantic parse + simple scene + egui
shim" into a product-grade, streaming, trust-aware, cross-surface document
surface. Defer the HTML lane; defer `graphshell-gpu` extraction; keep
forward-compatible seams for both.

**Related docs**:

- [`../technical_architecture/2026-04-16_middlenet_lane_architecture_spec.md`](../technical_architecture/2026-04-16_middlenet_lane_architecture_spec.md)
  — lane architecture invariants, crate topology, canonical types, lifecycle
- [`../technical_architecture/2026-03-29_middlenet_engine_spec.md`](../technical_architecture/2026-03-29_middlenet_engine_spec.md)
  — baseline Middlenet scope
- [`../research/2026-04-16_rendering_architecture_vision.md`](../research/2026-04-16_rendering_architecture_vision.md)
  — anyrender / WebRender future vision (not on v1.5 critical path)
- [`../research/2026-04-14_wasm_portable_renderer_feasibility.md`](../research/2026-04-14_wasm_portable_renderer_feasibility.md)
  — WASM envelope constraints
- [`viewer/universal_content_model_spec.md`](viewer/universal_content_model_spec.md)
  — viewer routing and content selection policy
- [`viewer/viewer_presentation_and_fallback_spec.md`](viewer/viewer_presentation_and_fallback_spec.md)
  — fallback/degraded-state expectations
- [`../../verso_docs/research/2026-04-16_smolnet_capability_model_and_scroll_alignment.md`](../../verso_docs/research/2026-04-16_smolnet_capability_model_and_scroll_alignment.md)
  — transport/format adapter split

---

## 1. Problem Statement

Middlenet's semantic seam is correct (see lane spec), but the Direct Lane is
still batch-only parse, placeholder `DocumentDelta` (`Replace` variant only),
default-Unknown trust, and a single-type document model where transport
state and content truth are tangled. `viewer:middlenet` renders but lacks
find-in-page, outline navigation, a11y projection, trust UI, and source-action
affordances.

Before starting `middlenet-html`, the Direct Lane must become strong enough
that HTML is a scoped lane addition rather than a vacuum cleaner for unresolved
UX.

This plan defines v1.5 of the Direct Lane.

---

## 2. Decisions Locked In

### 2.1 `verso` is the cross-engine dispatcher

Verso (conceptual today; this plan makes it real enough to host the boundary)
dispatches requests across **Middlenet**, **Servo**, and **Wry**.

`middlenet-engine::LaneDecision` therefore shrinks to:

```
LaneDecision::{ Direct, Html, FaithfulSource, Unsupported }
```

`Servo` and `Wry` variants are removed from Middlenet. A new seam (see §4.7)
places cross-engine escalation in `verso`.

"HTML lane" is unambiguously "Middlenet-internal HTML via Blitz top-half +
WebRender." Handing off to Servo is not a Middlenet fallback; it's a Verso
decision.

### 2.2 PreparedDocument is the transport dossier; SemanticDocument is the distilled summary

**`PreparedDocument` owns (raw dossier)**:

- fetch timestamps (precise instants, refresh/cache state)
- redirect chain
- MIME / transport observations (headers, negotiated types)
- certificate / TOFU result (detailed cert state, prior key comparison)
- source health / freshness (last-modified, etag, staleness)
- raw source handle
- parse / adaptation warnings
- lane-selection inputs

**`SemanticDocument` owns (distilled, portable)**:

- canonical URI
- source kind
- user-visible trust summary (`Trusted` | `Tofu` | `Insecure` | `Broken` |
  `Unknown`)
- user-visible provenance summary (human label, coarse fetched-at string,
  source label)
- alternate links / source link / article link
- semantic content blocks

`SemanticDocument` is self-contained enough that renderers, caches, and
exports never need to reach for the dossier. `PreparedDocument` is
per-session; `SemanticDocument` is cacheable.

### 2.3 Trust UX: shell is the authority, viewer is the interpreter

**Shell-level owns**:

- trust policy
- trust state changes
- prompts / approvals
- persistence of certs / TOFU / exceptions
- global badges, notices, routing consequences

**Viewer-level owns**:

- explaining the current document's trust state
- showing provenance and degradation inline
- document-local actions (view source, show cert details, why did this degrade)
- making trust legible in context

Shell lives in `graphshell-comms` (policy/persistence) + Graphshell shell
chrome (badges/prompts/routing). Viewer lives in `viewer:middlenet` and is
rendered *through* `middlenet-render` as proper `RenderScene` content — not
ad-hoc UI.

### 2.4 Async model: hybrid

- `middlenet-core`, `middlenet-render`, `middlenet-engine` depend only on the
  `futures` crate and a local `CancelToken` trait. **No tokio types leak
  through their public API.**
- `middlenet-transport` is tokio-backed (because reqwest / hyper / rustls
  force the choice in practice) and provides a `CancelToken` impl.
- This leaves room for a future `middlenet-transport-wasm` sibling crate
  without touching core.

### 2.5 Accessibility target: AccessKit

AccessKit is the target. Its tree format is close enough to neutral that
targeting it is the broader-consumable thing; no separate neutral tree is
worth the maintenance.

### 2.6 `graphshell-gpu` is deferred, but forward-compatible types land in v1.5

`graphshell-gpu` is not extracted in v1.5. Three forward-compatible types live
in `middlenet-render` now and migrate cleanly when the crate is born:

- `FontHandle` — opaque id used by render scenes
- `ImageRef` — handle to a decoded image, resolution-independent
- `OffscreenTarget` — trait: "paint this `RenderScene` into a buffer"

Cross-surface reuse (observation cards, search snippets, hover previews,
feed tiles) in v1.5 is **ad-hoc Phase 1**; `graphshell-gpu` folds it into a
unified Phase 2 later.

---

## 3. Crate Topology

### 3.1 Before

```
middlenet-core        (SemanticDocument + format serializers; meta with transport state)
middlenet-adapters    (parse bodies into documents; batch-only)
middlenet-engine      (facade; LaneDecision includes Servo/Wry; batch adapt)
middlenet-render      (linear scene builder; RenderMode enum underused)
graphshell-comms      (transport primitives: identity, webfinger, misfin)
```

### 3.2 After (v1.5)

```
middlenet-core        (SemanticDocument, PreparedDocument, CancelToken, lifecycle)
middlenet-formats     (format (de)serializers: gemini/gopher/finger/markdown/feeds/html-import)
middlenet-adapters    (parse + streamed DocumentDelta stream per format)
middlenet-transport   (fetch + TOFU + freshness + cache + retry + offline; above graphshell-comms)
middlenet-engine      (facade; LaneDecision: Direct/Html/FaithfulSource/Unsupported)
middlenet-render      (scene builder + refusal rendering + forward types)
verso                 (NEW; cross-engine dispatch across Middlenet/Servo/Wry)
graphshell-comms      (unchanged; middlenet-transport depends on it)
```

Servo and Wry dispatch live in `verso`. Wry is particularly important
for iOS where WebKit is mandated and Servo is unavailable.

---

## 4. Type-Shape Changes

### 4.1 `PreparedDocument`

```rust
pub struct PreparedDocument {
    pub source: MiddleNetSource,
    pub document: SemanticDocument,   // may be Arc'd later; not in v1.5

    // Raw dossier
    pub fetch: FetchRecord {          // precise timestamps, refresh state
        pub fetched_at: SystemTime,
        pub refreshed_at: Option<SystemTime>,
        pub from_cache: bool,
    },
    pub redirects: Vec<RedirectHop>,
    pub transport_observations: TransportObservations {
        pub mime: Option<String>,
        pub negotiated_content_kind: Option<MiddleNetContentKind>,
        pub response_headers: Vec<(String, String)>,
    },
    pub cert_result: Option<CertResult>,   // TOFU state, prior-key comparison,
                                           // cert chain summary
    pub source_health: SourceHealth {
        pub last_modified: Option<SystemTime>,
        pub etag: Option<String>,
        pub stale: bool,
    },
    pub raw_source: Option<RawSourceHandle>,   // owned handle, not inline String
    pub adaptation_warnings: Vec<AdaptationWarning>,
    pub lane_inputs: LaneSelectionInputs,
}
```

### 4.2 `SemanticDocument`

Slim down `DocumentMeta`. Remove transport-state fields. Add distilled trust
and provenance summaries:

```rust
pub struct DocumentMeta {
    pub canonical_uri: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content_kind: MiddleNetContentKind,
    pub trust_summary: DocumentTrustState,      // user-visible coarse enum
    pub provenance_summary: ProvenanceSummary { // user-visible
        pub source_label: Option<String>,
        pub fetched_at_display: Option<String>, // coarse "2 minutes ago"
    },
    pub alternate_open_targets: Vec<LinkTarget>,
    pub raw_source_available: bool,              // affordance flag
    pub article_hint: Option<String>,
    pub feed_hint: Option<String>,
}
```

Removed from meta (moved to `PreparedDocument`): precise `fetched_at`,
`diagnostics` (adaptation warnings are transport-side), old duplicate
`DocumentProvenance`.

Format serializers (`to_html`, `to_gophermap`, `to_finger_text`, `to_gemini`,
`from_gemini`, etc.) move to `middlenet-formats`. `SemanticDocument` stops
carrying format concerns as methods.

### 4.3 `LaneRenderOutput`

Today's scene-or-note is replaced by an enum:

```rust
pub enum LaneRenderOutput {
    Scene { lane: LaneDecision, scene: RenderScene },
    Refused { lane: LaneDecision, reason: RefusalReason, scene: RenderScene },
    Failed  { lane: LaneDecision, error: String },
}

pub enum RefusalReason {
    TrustBroken(TrustBreakDetail),
    TrustInsecure,
    UserPolicyDisallowed,
    FaithfulSourceRequested,
    ContentUnsupported,
}
```

The refusal path **still produces a `RenderScene`**. `middlenet-render`
exposes `render_refusal(&RefusalReason, &RenderRequest) -> RenderScene`. The
viewer paints it identically to a normal scene — refusal is just different
content, not a different UI surface.

### 4.4 `RenderLifecyclePhase` and streaming

```rust
pub enum RenderLifecyclePhase {
    Started,
    Partial,
    Complete,
    Invalidated,
    Failed,
    Cancelled,
}

pub enum DocumentDelta {
    Replace(SemanticDocument),        // kept for initial + full-replace flows
    AppendBlock(SemanticBlock),
    ReplaceBlock { index: usize, block: SemanticBlock },
    RemoveBlock  { index: usize },
    MetaUpdate(DocumentMetaPatch),
    ProvenanceUpdate(ProvenanceSummary),
    TrustUpdate(DocumentTrustState),
    ResourceDiscovered(ResourceRef),  // images, stylesheets, linked assets
    ParseWarning(AdaptationWarning),
    LifecyclePhase(RenderLifecyclePhase),
}
```

Adapters return `impl Stream<Item = DocumentDelta>`. The engine threads a
`CancelToken` through every adapt / render call.

### 4.5 `CancelToken`

```rust
pub trait CancelToken: Clone + Send + Sync + 'static {
    fn is_cancelled(&self) -> bool;
    fn cancelled(&self) -> impl Future<Output = ()> + Send;
}
```

Core defines the trait. `middlenet-transport` implements it via
`tokio_util::sync::CancellationToken`. Test harnesses provide a trivial impl.

### 4.6 Forward-compatible render types

```rust
pub struct FontHandle(u64);
pub struct ImageRef(u64);

pub trait OffscreenTarget {
    fn paint(&mut self, scene: &RenderScene) -> Result<(), OffscreenError>;
    fn into_image(self) -> Result<RasterImage, OffscreenError>;
}
```

Live in `middlenet-render` now; migrate to `graphshell-gpu` in a later phase.

### 4.7 `verso` (minimal)

```rust
pub enum VersoEngineChoice { Middlenet, Servo, Wry }

pub trait VersoDispatcher {
    fn choose(&self, req: &VersoRequest) -> VersoEngineChoice;
}
```

`verso` is a thin scaffold in v1.5 — just enough to move the Servo /
Wry escalation decision out of `middlenet-engine`. Full Servo/Wry delegation
implementation is not in v1.5.

---

## 5. Ordered Milestone Steps

Steps are sequenced so that earlier steps don't need rework when later ones
land. Each step is one or more PRs; PR decomposition is a separate exercise.

### Step 1: Resolve PreparedDocument / SemanticDocument ownership

- Strip `DocumentMeta` to distilled fields (§4.2).
- Add `trust_summary` + `provenance_summary` to `DocumentMeta`.
- Introduce expanded `PreparedDocument` fields (§4.1) — still batch-filled
  for now; streaming lands in Step 3.
- Remove `DocumentProvenance` as a separate struct; fold into
  `ProvenanceSummary` on the slim side and `PreparedDocument.fetch` on the
  raw side.
- Update all call sites: renderers read `SemanticDocument` only; shell chrome
  reads `PreparedDocument`.

### Step 2: Extract `middlenet-formats` and `middlenet-transport`

- **`middlenet-formats`**: move `to_html`, `to_gophermap`, `to_finger_text`,
  `to_gemini`, `from_gemini`, RSS/Atom/JSON Feed parsers, Markdown parser,
  article/readability extraction. `SemanticDocument` loses all format methods.
- **`middlenet-transport`**: new crate. Depends on `graphshell-comms`. Owns
  fetch pipeline, TOFU decisions (delegates persistence to shell via a trait),
  freshness / cache / retry, offline-archive hydration, provenance stamping.
  Tokio-backed.
- `middlenet-adapters` shrinks to "glue format parsers to `DocumentDelta`
  streams."
- Update `middlenet-engine` to orchestrate the new seam.

### Step 3: Lifecycle, streaming, cancellation (feeds pilot)

- Add `RenderLifecyclePhase` + expanded `DocumentDelta` variants (§4.4).
- Add `CancelToken` trait to `middlenet-core`; tokio impl in
  `middlenet-transport`.
- Adapters return `Stream<Item = DocumentDelta>`.
- Implement streaming for **RSS first**, then Atom, then JSON Feed. Non-feed
  formats keep emitting a single `Replace` delta for now.
- Engine threads cancellation through `adapt_stream(...)` and
  `render_live(...)`.

### Step 4: Trust UX — shell authority, viewer interpreter

- Shell side (in `graphshell-comms` + shell chrome):
  - TOFU decision engine with persistence
  - cert/exception store
  - prompt/approval flow
  - global trust badges wired to `SemanticDocument.trust_summary`
  - routing consequences (e.g., broken-trust blocks pane entry unless user
    overrides)
- Viewer side (in `middlenet-render` + `viewer:middlenet`):
  - `render_refusal(...)` path producing `RenderScene`
  - inline trust explanation block, provenance display, degradation note
  - document-local actions: "view source", "show cert details",
    "why did this degrade?"
- `LaneRenderOutput` enum split (§4.3) lands here.

### Step 5: Product-grade `viewer:middlenet`

- Find-in-page
- Outline navigation driven by `RenderScene.outline`
- Copy actions (selection → clipboard with semantic-aware formatting)
- Per-run link hit targets (word-granularity `HitRegion`s, not block-level)
- Explicit source / article / open-externally actions
- Keyboard navigation: tab focus, arrow-key link traversal, escape to close,
  type-to-find
- **AccessKit tree projection** from `RenderScene` — first-class deliverable,
  not a TODO.
- Diagnostics / trust / degradation affordances wired to Step 4 output.

### Step 6: Cross-surface reuse (Phase 1, ad-hoc)

Drive the same `SemanticDocument → RenderScene` path through:

- observation cards (`RenderMode::Card`)
- search result snippets (`RenderMode::Card`)
- hover previews (`RenderMode::PreviewThumbnail`)
- feed entry tiles (`RenderMode::Card`)
- clip previews (`RenderMode::PreviewThumbnail`)

Integration is per-surface in v1.5. No `graphshell-gpu` extraction. The
forward-compatible render types (`FontHandle`, `ImageRef`, `OffscreenTarget`)
are used by each surface so the later Phase 2 migration is cheap.

### Step 7: `verso` scaffold; remove Servo/Wry from `middlenet-engine`

- Create `verso` crate with `VersoEngineChoice` and `VersoDispatcher`
  trait.
- Remove `Servo` and `Wry` variants from `LaneDecision`.
- Remove `supports_servo_lane` / `supports_wry_lane` from `HostCapabilities`
  (move to `VersoHostCapabilities`).
- Wire the shell's existing Servo/Wry routing through `verso` instead
  of through Middlenet's lane enum.
- Amend the lane architecture spec to document this change.

---

## 6. Dogfood Acceptance Criteria

### 6.1 Feeds pilot

- A popular public RSS feed (target: The Verge, if their feed is published at
  a discoverable standard path) streams into a Middlenet feed-tile pane.
- Entries appear live as deltas arrive (not batch on complete).
- An Atom feed (e.g., a GitHub releases feed) and a JSON Feed (any reference
  example) also stream correctly.
- Cancellation: closing the pane or navigating away stops pending fetches
  within one frame.

### 6.2 Trust UX

- Fresh Gemini capsule: shell shows TOFU-new badge; viewer renders normally
  with inline TOFU explanation block; user can accept via shell prompt.
- Changed-key capsule: shell refuses by default; viewer shows refusal scene
  explaining why, offering "show cert details" and "view raw source" actions.
- Insecure HTTP: shell shows insecure badge; viewer renders normally with
  inline insecure notice.
- Parse degradation: viewer shows inline "why did this degrade?" with the
  specific `AdaptationWarning` list.

### 6.3 Cross-surface reuse

- Same `SemanticDocument` renders coherently as FullPage, Card, and
  PreviewThumbnail across observation cards, search snippets, hover previews,
  feed tiles.
- Trust state visible at every size.

### 6.4 a11y

- AccessKit tree projection covers all `RenderScene` content.
- Screen reader can navigate by heading, link, and block.
- Keyboard-only navigation works across all interactive affordances.

### 6.5 Lifecycle discipline

- All six `RenderLifecyclePhase` values are observable from a test harness.
- `Cancelled` is reachable via closing a pane or dropping a `CancelToken`.
- `Invalidated` fires on transport-side updates (e.g., new feed entry,
  cert-exception change) without tearing down the scene.

---

## 7. Out of Scope for v1.5

- **HTML lane** (`middlenet-html`). Deferred until Direct is strong enough
  that HTML is a scoped lane addition, per lane spec §11.
- **`graphshell-gpu` extraction.** Forward types live in `middlenet-render`
  until Phase 2.
- **Full Servo/Wry delegation.** `verso` is scaffold-only; the
  in-depth Servo lane implementation and Wry-for-iOS work are separate
  milestones.
- **Full Cargo-feature envelope gating** (lane spec §7.3). v1.5 assumes a
  native desktop envelope. WASM/PWA envelopes arrive in a later phase, with
  the async-model split (§2.4) designed to make the transport swap possible.
- **`PreparedDocument.document` as `Arc<SemanticDocument>`.** Kept as owned
  in v1.5; upgrade to Arc when the first real caching surface demands it.

---

## 8. Risks and Mitigations

### 8.1 Streaming deltas without lifecycle becomes a rewrite

*Mitigation*: §4.4 lands streaming and lifecycle together in Step 3. Don't
split them.

### 8.2 Shell/viewer trust boundary leaks

If the viewer starts persisting cert exceptions, or the shell starts
rendering inline explanations, the clean authority/interpreter split erodes.

*Mitigation*: Step 4 explicitly scopes each side. Code review gate: no
persistence calls from viewer code; no per-document rendering from shell
chrome.

### 8.3 `middlenet-formats` grows into a mini-browser

Format serializers are thin; but article/readability extraction and rich HTML
import could balloon.

*Mitigation*: `middlenet-formats` is format decoders only. If article-mode
extraction needs Stylo, that's `middlenet-html` territory, not `-formats`.

### 8.4 Cross-surface reuse copy-paste in Step 6

Without `graphshell-gpu`, each surface wires its own `render_document` call.
Drift risk.

*Mitigation*: A shared helper module in `middlenet-render` exposes
`render_for_surface(...)` taking the target surface as input. Five callers,
one helper. When `graphshell-gpu` is extracted, this helper migrates.

### 8.5 AccessKit projection lag

AccessKit trees need updating on every render change; doing this per-delta
could be expensive.

*Mitigation*: Project the tree once per `RenderLifecyclePhase::Complete` and
once per significant `Partial` batch (not per individual delta).

---

## 9. Step Dependencies

```
Step 1 (ownership) ──┬──> Step 2 (crate split) ──> Step 3 (streaming) ──┐
                     │                                                    │
                     └──> Step 4 (trust UX) <──────────────────────────── ┤
                                    │                                     │
                                    v                                     │
                              Step 5 (viewer v1.5) <──────────────────────┤
                                                                          │
                              Step 6 (cross-surface) <────────────────────┤
                                                                          │
                              Step 7 (verso scaffold) <───────────────────┘
```

Steps 1–3 are linear. Steps 4–7 can partially parallelize once 3 is done.
Step 7 (Verso scaffold) is independent of Steps 4–6 and can run in parallel
once Step 1 is done.

---

## 10. Definition of Done

v1.5 is complete when:

1. `middlenet-core` carries no transport state, no format serializers.
2. `middlenet-transport` + `middlenet-formats` exist and own their concerns.
3. `middlenet-engine::LaneDecision` is Direct / Html / FaithfulSource /
   Unsupported only.
4. RSS, Atom, JSON Feed stream deltas end-to-end, observable in the feed-tile
   pane.
5. Trust UX satisfies §6.2 across TOFU, cert-break, insecure, and degraded
   paths.
6. `viewer:middlenet` has find-in-page, outline, copy, per-run hit targets,
   keyboard navigation, AccessKit projection, and document-local trust
   actions.
7. Observation cards, search snippets, hover previews, and feed tiles share
   the `SemanticDocument → RenderScene` path.
8. `verso` exists as a crate; Servo/Wry routing flows through it.
9. Forward-compatible `FontHandle` / `ImageRef` / `OffscreenTarget` are in
   use across surfaces.
10. Lane architecture spec is amended to reflect the Verso boundary.

---

## 11. Follow-on Milestones (not this plan)

- **Direct Lane v2**: `graphshell-gpu` extraction; unified offscreen worker
  pool; `Arc<SemanticDocument>` caching; full Cargo-feature envelope gating.
- **HTML Lane v0.1**: `middlenet-html` with Blitz top-half (blitz-dom,
  blitz-html, Stylo, Taffy, Parley) and WebRender paint.
- **Verso v1**: full Servo and Wry delegation with host-capability probing.
- **Native feed operations**: subscriptions, source-health tracking,
  provenance-aware feed ranking.
- **Graph-native semantic indexing**: index `SemanticDocument`, not HTML
  strings.
- **`middlenet-transport-wasm`**: runtime-agnostic transport for WASM
  envelopes.
