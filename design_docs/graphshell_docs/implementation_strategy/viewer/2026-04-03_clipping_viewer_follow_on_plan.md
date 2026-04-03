# Clipping Viewer Follow-On Plan

**Date**: 2026-04-03
**Status**: Active follow-on plan
**Phase**: Viewer lane, post-landing refinement
**Architecture**: Graphshell-owned inspector and clip materialization remain landed; this plan covers the remaining viewer-lane refinement work only

**Related**:

- `clipping_and_dom_extraction_spec.md`
- `VIEWER.md`
- `../graph/2026-03-11_graph_enrichment_plan.md`
- `../aspect_projection/ASPECT_PROJECTION.md`
- `../aspect_distillery/ASPECT_DISTILLERY.md`
- `../system/2026-03-12_architectural_inconsistency_register.md`
- `../../../archive_docs/checkpoint_2026-04-03/graphshell_docs/implementation_strategy/viewer/2026-02-11_clipping_dom_extraction_plan.md`

---

## Summary

The prior clipping execution slice is now landed and archived as a current-state receipt. This follow-on plan exists to keep the remaining viewer-lane work explicit without reopening the already-completed core clip architecture.

This plan does **not** re-propose:

- inspector-first capture as the canonical complex-page flow,
- route-backed clip identity,
- persisted clip-content facet bridge state,
- snapshot-safe clip identity preservation,
- clip re-open behavior into the matching clip node pane,
- visible-title/source presentation cleanup across omnibar, search, and workbench surfaces.

Those are treated as current baseline.

---

## Baseline Already Landed

The active runtime already provides:

- Graphshell-owned clip inspector state and panel,
- single and batch clip materialization,
- pointer-stack inspection and stepped candidate selection,
- route-backed clip nodes via `Address::Clip(...)` / `AddressKind::GraphshellClip`,
- clip-content persistence through an explicit clip-content facet bridge,
- runtime viewer display synthesized from stored clip HTML,
- snapshot restore that preserves clip route identity,
- clip-route open behavior that resolves to the matching clip node pane,
- user-facing clip labeling/search behavior that prefers visible clip metadata over internal route identity.

This follow-on plan only addresses what remains after that baseline.

---

## Scope

### In scope

- inspector ergonomics beyond the first landed panel/pointer-stack flow,
- richer clip fidelity and capture-mode choices,
- stronger extraction robustness on complex page structures,
- clip-content storage and route cleanup once the bridge period can tighten,
- richer clip presentation and provenance chrome.

### Out of scope

- site-wide link harvesting,
- selector-recipe batch analysis as a graph mutation system,
- projection of full page structure into graph truth by default,
- typed distillation/extraction artifacts beyond the clip-local viewer lane,
- Nostr/highlight publication as a driver of viewer-lane architecture.

---

## Remaining Viewer-Lane Targets

### 1. Inspector ergonomics

The current panel works, but it still feels like a low-level probe surface.

Follow-on goals:

- reduce pointer-stack friction on dense pages,
- improve candidate naming and preview affordances,
- make candidate filtering and narrowing more legible,
- preserve a fast one-hand path from inspect to materialize.

Acceptance direction:

- a user can move from "wrong inner span" to the intended container with fewer manual stepping actions,
- the panel exposes enough preview context that users can choose without trial-and-error clipping.

### 2. Clip fidelity choices

The current system creates useful clips, but it still treats fidelity as mostly one shape.

Follow-on goals:

- define concrete capture modes such as `Clean`, `Contextual`, `Screenshot Note`, and `Offline Slice`,
- keep the modes viewer-owned and explicit rather than heuristic-only,
- avoid turning fidelity choice into a hidden persistence or publication policy.

Acceptance direction:

- a user can intentionally choose whether the clip should optimize for readability, surrounding context, visual fidelity, or offline survivability.

### 3. Extraction robustness

The extraction heuristics are broader than before, but they are still not the end-state.

Follow-on goals:

- improve behavior on deeply nested or component-heavy pages,
- tighten fallback behavior when extracted HTML is not independently readable,
- keep extraction read-only and viewer-owned rather than drifting into a general analysis subsystem.

Acceptance direction:

- common complex pages produce a materially higher first-try success rate for intended clip boundaries,
- failed/self-broken clips degrade predictably instead of silently producing low-value artifacts.

### 4. Clip storage and route cleanup

The bridge period is acceptable, but not final.

Follow-on goals:

- tighten the route family after the legacy alias can retire,
- clarify whether clip HTML remains node-owned state or moves behind a stronger storage reference,
- keep `#clip` as a derived compatibility surface rather than long-term content authority.

Acceptance direction:

- the route/render/storage story becomes simpler without regressing existing clip open/display behavior,
- clip semantics depend on explicit facet/state carriers rather than tag-shaped authority.

### 5. Presentation and provenance chrome

User-visible labels are cleaned up, but clip-specific presentation is still modest.

Follow-on goals:

- improve provenance display inside clip presentation surfaces,
- expose clearer capture metadata where it helps retrieval and trust,
- make clip artifacts feel intentionally distinct without turning them into a separate top-level node kind.

Acceptance direction:

- users can tell what was clipped, from where, and with what capture context without reading internal identifiers or opening inspector/debug views.

---

## Execution Slices

### Slice A: Inspector usability pass

- improve candidate preview labels,
- reduce pointer-stack stepping friction,
- refine panel actions around select/materialize.

### Slice B: Explicit fidelity mode pass

- define the first supported fidelity modes,
- thread those modes through clip creation and rendering,
- keep storage/provenance behavior explicit per mode.

### Slice C: Extraction hardening pass

- expand real-page coverage,
- improve fallback behavior for brittle/self-broken clips,
- add focused validation cases for complex layouts.

### Slice D: Bridge cleanup pass

- tighten route-family compatibility,
- settle the next clip-content storage shape,
- keep `#clip` compatibility derived from facet/state truth.

### Slice E: Presentation chrome pass

- enrich provenance display,
- add clip-specific chrome only where it improves retrieval and trust,
- preserve the current user-visible title/source cleanup as baseline.

---

## Validation

1. The archived 2026-02-11 clipping plan remains the record of the landed current-state slice.
2. This plan contains only remaining viewer-lane work rather than re-documenting already-landed architecture.
3. A reader can distinguish inspector ergonomics, fidelity choice, extraction hardening, bridge cleanup, and presentation chrome as separate follow-on targets.
4. The plan does not silently re-expand clipping back into a general document-analysis or publication architecture.
5. The current route/facet/visible-metadata baseline remains treated as settled starting point for this next pass.

---

## Defaults

- Default baseline: current clip inspector/materialization architecture is landed.
- Default next step: refine ergonomics and fidelity before reopening deeper storage cleanup.
- Default boundary: viewer lane owns clip capture and presentation refinement only.
- Default downstream stance: projection, enrichment, distillery, and publication remain separate lanes that consume clip artifacts rather than redefine this viewer plan.