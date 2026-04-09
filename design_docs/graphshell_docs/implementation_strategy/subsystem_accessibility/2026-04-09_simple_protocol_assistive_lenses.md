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