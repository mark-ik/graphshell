<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Simple-Protocol Assistive Lenses

**Date**: 2026-04-09
**Status**: Implementation strategy / Track A follow-on
**Scope**: Define optional assistive lenses for Gemini, Gopher, feeds, and
other simple protocols while preserving the faithful source render as the
default truth.

**Related**:

- [SUBSYSTEM_ACCESSIBILITY.md](SUBSYSTEM_ACCESSIBILITY.md)
- [accessibility_interaction_and_capability_spec.md](accessibility_interaction_and_capability_spec.md)
- [../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md](../../research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md)
- [../../research/2026-04-09_smolweb_browser_capability_gaps.md](../../research/2026-04-09_smolweb_browser_capability_gaps.md)

---

## 1. Core Rule

Graphshell should preserve the source protocol grammar by default and layer
assistive structure on top as an explicit mode.

Therefore:

- Gemini renders as Gemini,
- Gopher renders as Gopher,
- feeds render as feeds,
- assistive lenses are named, inspectable overlays rather than silent
  replacements.

This is the required accessibility posture for simple protocols.

---

## 2. Lens Types

The first assistive-lens family should include:

- document outline lens,
- heading/section inventory,
- action and link-role summary,
- feed summary lens,
- thread-summary lens,
- speech-friendly or low-distraction view,
- ASCII/icon fallback where decorative glyphs reduce clarity,
- graph-aware provenance summary.

These are assistive overlays, not new protocol semantics.

---

## 3. Labeling Rule

Users must be able to tell whether a surface is:

- faithful,
- assistive,
- summarized,
- transformed.

The same requirement applies to screen-reader and non-visual consumers: the
mode must be discoverable rather than inferred from changed output.

---

## 4. Capability Contract

Each assistive lens should declare:

- which source protocols it can operate on,
- whether it preserves full source access,
- what additional structure it adds,
- what degradation mode applies when the lens cannot be computed.

This keeps the subsystem aligned with the broader Accessibility capability
model.

---

## 5. First Slice

Recommended first slice:

1. heading/outline lens for gemtext and Markdown-like content,
2. page or capsule structure inventory,
3. feed summary lens,
4. speech-friendly low-distraction mode,
5. explicit mode labeling and return-to-faithful action.

This is enough to improve accessibility without accidentally replacing the
source protocol with a richer private format.

---

## 6. Implementation Slices

### Slice A: Assistive Lens Contract

- define a typed assistive-lens descriptor,
- record supported protocols, added structure, and degradation behavior,
- ensure each lens declares whether faithful source access remains available.

### Slice B: Outline and Structure Lenses

- add heading or section inventory for gemtext and Markdown-like content,
- expose link/action summaries where structure is otherwise implicit,
- preserve return-to-faithful behavior.

### Slice C: Feed and Thread Summaries

- add compact assistive summaries for feed-like and thread-like content,
- ensure these are clearly labeled as summaries or assistive modes,
- avoid silently transforming the canonical source representation.

### Slice D: Accessibility Mode Surfacing

- show current mode as faithful, assistive, summarized, or transformed,
- ensure screen-reader and keyboard paths can discover the active mode,
- degrade cleanly when a lens cannot be computed.

---

## 7. Validation

### Manual

1. Open a Gemini or feed document and switch into an assistive lens.
2. Verify the active mode is labeled and reversible.
3. Verify the faithful source render remains accessible.
4. Verify unsupported lenses degrade honestly instead of appearing broken.

### Automated

- descriptor tests for protocol support and degradation metadata,
- rendering tests for outline and summary modes,
- accessibility regressions for mode labeling and keyboard discovery.

---

## 8. Done Gate

This slice closes when:

- assistive lenses are explicit typed overlays,
- at least one outline/structure lens and one summary lens are usable,
- mode labeling is visible and non-visual-accessible,
- and faithful protocol rendering remains the default truth.