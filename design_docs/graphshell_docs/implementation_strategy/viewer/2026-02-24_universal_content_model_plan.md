# Universal Node Content Model: Implementation Strategy

**Date**: 2026-02-24
**Status**: Active. Track A is implementation-ready; Track B is follow-on architecture and should not be treated as committed runtime scope yet.
**Relates to**:

- `../../technical_architecture/2026-02-18_universal_node_content_model.md` - research vision document
- `../../technical_architecture/2026-03-29_middlenet_engine_spec.md` - current architectural authority for the shared document-model and adaptation direction behind Track B
- `universal_content_model_spec.md` - canonical contract for node content fields, `Viewer`, selection policy, MIME detection, sandboxing, and core/host split
- `viewer_presentation_and_fallback_spec.md` - canonical viewer/fallback semantics and baseline viewer taxonomy
- `wry_integration_spec.md` - active Wry contract; supersedes the older strategy doc
- `2026-03-08_servo_text_editor_architecture_plan.md` - edit-intent selection rule and editor exemption from reader adaptation
- `../system/register/viewer_registry_spec.md` - registry ownership boundary
- `../system/2026-02-21_lifecycle_intent_model.md` - lifecycle promotion/demotion integration
- `../graph/node_badge_and_tagging_spec.md` - badge and tag surfaces that consume content classification
- `../graph/semantic_tagging_and_knowledge_spec.md` - semantic tag suggestion and knowledge linkage
- `../graph/layout_behaviors_and_physics_spec.md` - current canvas clustering reference; replaces the older layout-behaviors plan link
- `2026-03-02_filesystem_ingest_graph_mapping_plan.md` - blocked on viewer readiness and `FilePermissionGuard`
- `2026-03-05_node_viewport_preview_minimal_slice_plan.md` - preview/thumbnail integration

---

## Context

The research vision still holds: a Graphshell node is a persistent, addressable content container,
not a browser tab. Renderers are attached views over node content, not the node's identity.

What changed since the earliest draft of this plan is that the surrounding viewer architecture has
become much more explicit. We now have canonical specs for:

- node content fields and viewer selection policy,
- viewer presentation and fallback semantics,
- Wry integration boundaries,
- edit-intent routing for text editing.

That means this document should no longer try to be both the canonical contract and the
implementation sequence. Its job is narrower:

1. sequence the work,
2. identify what is actually ready to build now,
3. keep follow-on architecture visible without pretending it is already implementation-ready.

---

## Critical Corrections From The Prior Draft

The earlier version of this plan had several useful ideas, but it had drifted in a few important
ways. This revision makes those corrections explicit:

1. **Status is no longer blanket "Implementation-Ready".**
   The non-web viewer foundation is ready. The shared `SimpleDocument` / `EngineTarget` /
   Servo-adaptation architecture is still design work.
2. **Selection policy now defers to the canonical UCM spec.**
   The fallback floor is `viewer:fallback`, not `viewer:plaintext`, and there is no blanket
   "File(html) -> webview" fallback in this plan.
3. **Wry references now point to the active spec.**
   The old `2026-02-23_wry_integration_strategy.md` path is gone; `wry_integration_spec.md` is
   the active contract.
4. **Broken and superseded related-doc links are fixed.**
   The old layout-behaviors plan link is replaced with
   `layout_behaviors_and_physics_spec.md`.
5. **The old missing `2026-03-08_simple_document_engine_target_spec.md` placeholder is replaced by
   Middlenet alignment.**
   Track B should align to `2026-03-29_middlenet_engine_spec.md`, and any narrower viewer/UCM
   contract should be extracted from that architecture explicitly instead of pointing at a missing file.

---

## Canonical Ownership Boundary

This plan is not the authority for every concept it mentions.

Use the following ownership model:

- `universal_content_model_spec.md` owns:
  - `mime_hint`
  - `AddressKind`
  - `viewer_override`
  - `Viewer` trait semantics
  - `ViewerRegistry` selection policy
  - MIME detection order
  - security/sandboxing invariants
  - core/host split
- `viewer_presentation_and_fallback_spec.md` owns:
  - baseline viewer taxonomy
  - fallback and degraded-state meaning
  - render-mode presentation semantics
- `wry_integration_spec.md` owns:
  - Wry backend behavior
  - overlay vs texture rules
  - Wry lifecycle and backend selection semantics
- this plan owns:
  - sequencing
  - dependency order
  - crate choices
  - done gates
  - what belongs in Track A versus Track B

---

## The Stable Data-Model Change

The implementation-ready schema change remains intentionally small:

```rust
pub mime_hint: Option<String>;
pub address_kind: AddressKind;
```

What does not change in this slice:

- `Node.url: String` remains the current durable address field.
- `Node.id: Uuid` remains the stable identity.
- `viewer_override: Option<ViewerId>` remains the explicit user choice surface.

Why this stays small:

- the typed `Address` enum from the research note is still a good long-term direction,
- but it is a separate schema migration,
- and it is not required to ship non-web viewers, viewer selection, or filesystem browse/read
  behavior now.

So this plan keeps `Node.url + AddressKind + mime_hint` as the non-breaking implementation slice.

---

## Track Split

### Track A: Implementation-Ready Foundation

This is the slice that should be treated as real build work now:

1. node content fields and WAL support,
2. viewer selection policy in runtime paths,
3. baseline non-web viewers,
4. security and file-permission enforcement,
5. badge/tag integration,
6. optional feature-gated PDF and audio viewers.

Track A is enough to unlock meaningful local-file and common-document coverage, and it is the
actual prerequisite for filesystem ingest.

### Track B: Follow-On Rich-Document Architecture

This is still valuable, but it should be treated as design/prototyping work until its contracts
are split out and accepted:

1. Gemini as a first-class resolver path,
2. explicit renderer-family switching (`Servo | Wry | Viewer`),
3. a shared document/profile model,
4. Servo-first adaptation pipeline,
5. capability packs and evidence-gated adoption policy.

Track B must not block Track A shipping.

---

## Track A Implementation Plan

### Step 1: Node Content Fields And WAL

**Goal**: `Node` carries `mime_hint` and `address_kind`, and both survive persistence.

Work:

- add `mime_hint: Option<String>` and `address_kind: AddressKind` to the node model,
- add WAL/update-intent coverage for both fields,
- perform cheap MIME inference at node creation time,
- perform higher-confidence detection when content bytes become available,
- write back improved MIME results once, not per frame.

Implementation notes:

- follow the canonical MIME detection order from `universal_content_model_spec.md`,
- treat `AddressKind` as authoritative for dispatch until the typed-address migration exists,
- classify directories during address resolution rather than guessing from syntax alone.

**Done gate**:

- `file:///foo.pdf` resolves to `AddressKind::File` and `mime_hint = application/pdf`,
- a directory resolves to `AddressKind::Directory`,
- `https://example.com` resolves to `AddressKind::Http`,
- WAL replay restores both fields deterministically.

---

### Step 2: ViewerRegistry Selection In Runtime Paths

**Goal**: node activation uses the canonical viewer-selection contract rather than ad hoc routing.

Work:

- implement the selection policy from `universal_content_model_spec.md`,
- route lifecycle promotion through `ViewerRegistry` resolution,
- preserve `viewer_override` as the highest-priority explicit user choice,
- store the resolved viewer on the live node-pane/viewer runtime path rather than recomputing it
  each frame.

Critical policy note:

- the fallback floor is `viewer:fallback`,
- `viewer:plaintext` is a real viewer, not the universal unsupported-content floor,
- `Http`/`Data` and `GraphshellClip` have direct dispatch rules,
- edit-intent text routing belongs to `viewer:text-editor`,
- this plan does not add a blanket `File(html) -> viewer:webview` fallback.

**Done gate**:

- selection results match the canonical spec for override, edit-intent, MIME-claimed viewers,
  detection-claimed viewers, and fallback,
- a PDF node resolves to `viewer:pdf` only when that viewer is registered,
- an unsupported file resolves to `viewer:fallback`,
- existing Servo `Http` routing does not regress.

---

### Step 3: PlaintextViewer

**Goal**: ship the first real non-web viewer and prove the embedded-viewer contract end to end.

Work:

- fully implement `viewer:plaintext`,
- support `text/*` plus common structured text formats (`json`, `toml`, `yaml`, `csv`),
- keep it read-only,
- render markdown without routing through Servo,
- provide a binary-safe fallback instead of panicking on non-text content.

Implementation guidance:

- use `pulldown-cmark` as the baseline Markdown parser,
- use `egui_commonmark` only if it is confirmed compatible with the current egui version,
- use `syntect` with the `fancy-regex` backend for the read-only path,
- cache expensive syntax/theme initialization behind `OnceLock`,
- keep `tree-sitter` concerns in `viewer:text-editor`, not here.

**Done gate**:

- text files render in a workbench tile,
- markdown has a real read path,
- syntax highlighting works for the curated language subset,
- binary files degrade gracefully instead of crashing.

---

### Step 4: ImageViewer

**Goal**: support common image content as a native embedded viewer.

Work:

- implement `viewer:image`,
- support raster formats already covered by the `image` crate,
- support SVG through `resvg`,
- handle animated GIFs with reduced-motion awareness,
- integrate loading/error states cleanly with viewer fallback presentation.

Implementation guidance:

- image decode must happen off the frame thread,
- keep a bounded texture cache,
- push preview/thumbnail invalidation through the preview system rather than inventing a
  second thumbnail path here.

**Done gate**:

- PNG/JPEG/WebP render in node panes,
- SVG renders through `resvg`,
- loading/error/degraded states are explicit,
- large-image memory use is bounded by cache policy.

---

### Step 5: DirectoryViewer

**Goal**: support browse-in-place local directory navigation without conflating it with ingest.

Work:

- implement `viewer:directory` for `AddressKind::Directory`,
- show local directory entries,
- navigate the current node when a directory entry is opened,
- create a new node when a file is dragged to the graph,
- keep state ephemeral per active node/view.

Critical boundary:

- `DirectoryViewer` is browse-in-place,
- bulk import remains the responsibility of the filesystem ingest plan,
- this step is still a prerequisite for that later ingest UX because it proves local directory
  navigation and permission handling.

**Done gate**:

- a local directory opens into a directory viewer,
- clicking a child file navigates the node to `file://...`,
- dragging a file to the graph creates a node,
- no directory state leaks between nodes after detach.

---

### Step 6: Security And File Permission Guard

**Goal**: make local-file viewing safe enough to ship before broader filesystem features open up.

Work:

- implement `FilePermissionGuard`,
- route all non-web local-file reads through it,
- define allow/deny/prompt behavior in app preferences,
- keep non-web viewers network-silent,
- treat permission denials as explicit viewer-surface outcomes, not silent failures.

Critical sequencing note:

- this is a hard prerequisite for the filesystem ingest plan,
- it should land before any feature that increases file-surface breadth.

**Done gate**:

- out-of-scope file access prompts or denies according to policy,
- denied access resolves to an explicit unsupported/permission surface,
- non-web viewers do not initiate network requests,
- contract tests cover allowed, denied, and prompted paths.

---

### Step 7: Badge And Tag Integration

**Goal**: surface content classification in graph-facing chrome after viewer selection is real.

Work:

- add content-type badge support based on resolved viewer/content classification,
- connect MIME/address-kind information to tag-suggestion surfaces,
- keep this derived from node content facts rather than inventing separate badge-local state.

Implementation guidance:

- keep badge priority aligned with the badge spec,
- do not let badge logic become a second selection system,
- clip-node and file-node semantics should remain distinct.

**Done gate**:

- PDF/image/text/audio/directory nodes can surface distinct content badges where appropriate,
- tag suggestions can consume `mime_hint`,
- badge ordering remains consistent with the badge spec.

---

### Step 8: PdfViewer (Feature-Gated)

**Goal**: add native PDF viewing without making it a baseline runtime dependency.

Work:

- implement `viewer:pdf` behind `--features pdf`,
- use `pdfium-render`,
- keep viewer registration absent when the feature is off,
- degrade to `viewer:fallback` when PDF support is not compiled in.

Critical policy note:

- do not treat PDF support as part of the baseline viewer floor,
- do not use `mupdf-sys`,
- if PDFium packaging remains too painful, this step can stay deferred without blocking Track A.

**Done gate**:

- `cargo build --features pdf` succeeds,
- PDF nodes resolve to `viewer:pdf` only when compiled in,
- builds without `pdf` still compile cleanly and resolve PDF files to explicit fallback.

---

### Step 9: AudioViewer (Feature-Gated)

**Goal**: add minimal local audio playback without broadening the baseline surface too early.

Work:

- implement `viewer:audio` behind `--features audio`,
- decode with `symphonia`,
- play back with `rodio`,
- expose minimal transport controls.

Critical policy note:

- audio is useful, but it is not a prerequisite for universal content model viability,
- keep it optional until platform behavior is proven.

**Done gate**:

- `cargo build --features audio` succeeds,
- local audio playback works in a tile,
- builds without `audio` still compile cleanly.

---

## Track B Follow-On Architecture

Track B remains worth pursuing, but it should not be described as "already implementation-ready."
Its first requirement is a contract boundary cleanup.

### Track B Gate: Align UCM Track B With Middlenet Canonical Contracts

Before implementing the shared adaptation path, align Track B with
`../../technical_architecture/2026-03-29_middlenet_engine_spec.md`, which is now the architectural
authority for the shared intermediate document-model and adaptation direction.

Legacy names from earlier UCM drafts:

- `SimpleDocument`
- `EngineTarget`
- `RenderPolicy`

These may still be useful local terms, but they should not be treated as already-canonical
standalone contracts. If Track B needs narrower viewer-facing contracts, extract them explicitly
from the Middlenet architecture into a real spec or into focused sections of
`universal_content_model_spec.md` / `VIEWER.md`.

**Done gate**:

- Track B terminology points at the Middlenet engine spec or an explicitly extracted derivative,
- there is no remaining dependency on the missing `2026-03-08_simple_document_engine_target_spec.md`,
- any viewer-facing shared-document contract exists as a real published document rather than a placeholder reference.

---

### Step 10: Gemini Protocol Resolver

**Goal**: support `gemini://` content without smuggling protocol work into the baseline file-viewer slice.

Work:

- add a feature-gated Gemini resolver,
- parse Gemini response headers and content types,
- render Gemini through a text/reader-oriented viewer path,
- keep privacy behavior narrow and explicit.

Why this is Track B:

- Gemini is valuable, but it is not required for the immediate local-file and common-document
  viewer floor,
- once reader-mode/shared-document work exists, Gemini likely wants to plug into that path rather
  than live forever as an isolated one-off renderer.

**Done gate**:

- feature-gated Gemini build succeeds,
- a known capsule renders through the chosen reader path,
- links navigate cleanly.

---

### Step 11: Explicit Render Selection And Shared Viewer Profiles

**Goal**: let users choose renderer families directly and make reader-style rendering an explicit
mode rather than a hidden fallback trick.

Work:

- add `Render With -> Servo | Wry | Viewer`,
- show current renderer family and unavailable reasons,
- introduce profile-level selection only after the shared-document contract exists.

Critical boundary:

- renderer-family choice is user-facing policy,
- it should not be smuggled into fallback-only behavior,
- Wry remains a compatibility web path, not a universal ingest target for arbitrary viewer output.

**Done gate**:

- users can switch renderer families explicitly,
- availability is diagnosable,
- Viewer profile mode is not dependent on Wry being enabled.

---

### Step 12: Servo-First Content Adaptation Pipeline

**Goal**: maximize Servo usefulness for rich-document rendering without forcing every content type
through Servo.

Work:

- define the adaptation pipeline from source resolution to engine target,
- compile readerized/shared-document content to Servo-compatible output where that is the best fit,
- preserve a native reader path for deterministic low-surface rendering.

Critical policy:

- Servo-first for rich documents does not mean Servo-for-everything,
- text editing remains exempt and stays under the editor plan,
- adaptation success/failure needs diagnostics, not guesswork.

**Done gate**:

- at least one non-HTML adapted source renders through Servo via the shared target contract,
- diagnostics report which target path was used,
- recommendations to switch renderers are based on observed outcome data.

---

### Step 13: Capability Packs

**Goal**: keep the built-in Reader/Media model stable while allowing optional support depth to grow.

Work:

- define pack contribution contracts,
- keep built-in mode families canonical,
- let packs extend codecs, transforms, profiles, or backend support in explicit ways.

Critical policy:

- packs extend mode families; they do not replace the core semantics,
- registry declarations and diagnostics are required before packs become selectable.

**Done gate**:

- at least one reader-side and one media-side extension path are spec'd and selectable through
  registry metadata,
- availability and conformance are visible in diagnostics/UI.

---

### Step 14: Evidence-Gated Servo Adoption Policy

**Goal**: make Servo promotion decisions from fidelity/runtime evidence instead of ideology.

Work:

- define a scorecard for fidelity, efficiency, complexity, and reliability,
- collect comparable diagnostics across candidate content paths,
- mark each content/profile path as Servo-primary, dual-path, or non-Servo-primary based on data.

Critical policy:

- use Servo aggressively where it wins,
- do not force Servo into roles where it is materially worse,
- record why a path stays dual or native-primary.

**Done gate**:

- scorecards exist for the initial target families,
- at least one path is promoted to Servo-primary with evidence,
- at least one path remains dual or native-primary with explicit justification.

---

## Crate Summary

These choices remain reasonable, but they should be read in track context rather than as one
monolithic commitment.

| Crate | Track | Purpose | Notes |
| ----- | ----- | ------- | ----- |
| `infer` | A | magic-byte MIME detection | pure Rust |
| `mime_guess` | A | extension-based MIME detection | already in use |
| `syntect` | A | read-only syntax highlighting | use `fancy-regex` backend |
| `pulldown-cmark` | A | Markdown parse path | baseline parser |
| `resvg` | A | SVG rendering | pure Rust |
| `pdfium-render` | A optional | PDF rendering | native dependency; feature-gated |
| `symphonia` | A optional | audio decode | feature-gated |
| `rodio` | A optional | audio playback | feature-gated |
| `extism` | B later | WASM/plugin sandboxing for future extension paths | not a Track A blocker |

Crates to avoid or treat cautiously:

- `mupdf-sys` - AGPL risk
- any Markdown/viewer helper that lags the current egui version badly enough to become a forced
  fork/maintenance burden

---

## Feature Flag Summary

```toml
[features]
default = ["gamepad", "servo/clipboard", "js_jit", "max_log_level", "webgpu", "webxr", "diagnostics"]
pdf = ["dep:pdfium-render"]
audio = ["dep:symphonia", "dep:rodio"]
gemini = []
wry = ["dep:wry"]
```

Policy:

- Track A baseline remains useful without `pdf`, `audio`, `gemini`, or `wry`,
- feature-gated viewers must disappear cleanly from registration when disabled,
- runtime fallback behavior must remain explicit when a feature-gated viewer is absent.

---

## Execution Order

### Recommended Track A Sequence

`1 -> 2 -> 3`

Then in parallel where practical:

- `4` ImageViewer
- `5` DirectoryViewer
- `6` FilePermissionGuard

Then:

- `7` Badge/tag integration
- `8` PdfViewer (optional)
- `9` AudioViewer (optional)

Rationale:

- Step 3 proves the first real non-web viewer path,
- Steps 4-6 expand coverage and safety,
- Step 7 should wait until content classification is real,
- Steps 8-9 are useful but non-blocking.

### Recommended Track B Sequence

`Track B Gate -> 10 -> 11 -> 12 -> 13 -> 14`

Rationale:

- the shared-document contract must exist before the adaptation stack and capability-pack model
  harden,
- Gemini can still be prototyped early, but it should not bypass the contract cleanup.

---

## Risks And Mitigations

**Selection-policy drift**

- Risk: local implementation quietly diverges from the canonical viewer spec again.
- Mitigation: treat `universal_content_model_spec.md` as the source of truth and add contract tests
  around fallback, override, and edit-intent routing.

**PDFium distribution**

- Risk: native packaging burden outweighs the near-term value of `viewer:pdf`.
- Mitigation: keep PDF support feature-gated and explicitly deferrable.

**`syntect` build and binary cost**

- Risk: the full syntax bundle is heavier than the actual product value.
- Mitigation: use a curated subset and keep the highlighting surface read-only.

**Large-image memory pressure**

- Risk: decoded textures and SVG rasterization can bloat memory.
- Mitigation: bounded cache plus preview-system integration.

**Filesystem surface expansion before permission hardening**

- Risk: directory/file features outrun `FilePermissionGuard`.
- Mitigation: keep Step 6 ahead of ingest and any broader file-feature rollout.

**Track B overreach**

- Risk: the reader/adaptation stack blocks the simple viewer floor.
- Mitigation: keep Track A shippable on its own and require a real canonical contract before
  Track B code hardens.

---

## Findings

The universal content model remains a strong direction, but it is not one single implementation
slice.

The critical near-term insight is that Graphshell already has enough registry and viewer
infrastructure to ship a solid first non-web content floor without waiting for the richer
Servo-adaptation architecture. The immediate value is in:

- correct node content fields,
- deterministic viewer selection,
- plaintext/image/directory viewing,
- explicit fallback,
- file-permission enforcement.

The critical long-term insight is that the shared-document and Servo-first adaptation path is
promising, but still needs a cleaned-up contract boundary before it should be treated as committed
implementation work.

---

## Progress

### 2026-02-24

- Initial implementation strategy drafted from the research vision.
- Non-web viewer expansion, MIME hints, and address-kind routing were identified as the core
  delivery direction.

### 2026-04-03

- Refactored the plan into two tracks:
  - Track A: implementation-ready viewer floor
  - Track B: follow-on shared-document and adaptation architecture
- Corrected stale and broken related-doc references.
- Realigned selection/fallback language with `universal_content_model_spec.md` and
  `viewer_presentation_and_fallback_spec.md`.
- Replaced the missing `2026-03-08_simple_document_engine_target_spec.md` placeholder with
  explicit alignment to the Middlenet engine spec.
- Reordered the implementation sequence so `FilePermissionGuard` and the baseline viewers are
  clearly the immediate shipping path.
