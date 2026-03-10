# Planning Register Audit Cleanup Receipt

**Date**: 2026-03-10
**Scope**: Cosmetic/hygiene edits to `PLANNING_REGISTER.md` — no sequencing or policy changes.
**Auditor**: Claude Code (session audit)

---

## Changes Applied

### 1. §3 Quick Wins — Items 1, 2, 9 Marked as Done

Items 1, 2, and 9 were confirmed landed in code at the time of audit:
- Item 1 (`radial_menu.rs` extraction): file exists at `render/radial_menu.rs`
- Item 2 (`command_palette.rs` extraction): file exists at `render/command_palette.rs`
- Item 9 (`ChannelSeverity` on diagnostics channel descriptors): landed in diagnostics registry work

Each was given a `✅ Done` prefix and the "Why It Pays Off" cell updated to note closure.

### 2. §2 Forgotten Concepts Table — Orphaned Adoption Notes Folded In

Five ranks had "appended adoption note (pending table refactor)" comments at the bottom of §2
that were never merged into the table itself. These were folded into the table's "Source Docs" column
and the "Adoption Trigger" column was updated to reflect that each concept now has a strategy doc
and should be treated as adopted/tracked rather than forgotten.

Affected ranks: 1, 2, 3, 4, 8.

The trailing appended-note blocks were removed from the bottom of §2 after being absorbed.

---

## Original Text Preserved Below

### Original §3 Quick Wins Table (rows 1, 2, 9)

```markdown
| 1 | **Extract `desktop/radial_menu.rs` from `render/mod.rs`** | Reduces render module sprawl and unblocks control UI redesign without behavior changes. | `2026-02-24_control_ui_ux_plan.md` |
| 2 | **Extract `desktop/command_palette.rs` from `render/mod.rs`** | Same benefit as #1; clarifies ownership for unified command surface work. | `2026-02-24_control_ui_ux_plan.md` |
| 9 | **Add `ChannelSeverity` to diagnostics channel descriptors** | Small schema extension that unlocks better pane prioritization and health summary. | `2026-02-24_diagnostics_research.md` §4.6, §7 |
```

### Original §2 Forgotten Concepts Table (full)

```markdown
| Rank | Forgotten Concept | Adoption Value | Source Docs | Adoption Trigger |
| --- | --- | --- | --- | --- |
| 1 | **Ghost Nodes (nodes/edges preserved after deletion)** | Preserves structural memory and reduces disorientation after destructive edits. Previously "Visual Tombstones"; canonical term is now Ghost Node. Code-level state: `NodeLifecycle::Tombstone`. | `2026-02-24_visual_tombstones_research.md` | After traversal/history UI and deletion UX are stable. |
| 2 | **Temporal Navigation / Time-Travel Preview** | Makes traversal history and deterministic intent log materially useful to users (not just diagnostics). | `2026-02-20_edge_traversal_impl_plan.md` (Stage F), `GRAPHSHELL_AS_BROWSER.md`, `2026-02-18_graph_ux_research_report.md` | After Stage E History Manager closure and preview-mode effect isolation hardening. |
| 3 | **Collaborative Presence (ghost cursors, remote selection, follow mode)** | Turns Verse sync from data sync into shared work. | `2026-02-18_graph_ux_research_report.md` §15.2, `GRAPHSHELL_AS_BROWSER.md`, Verse vision docs cited there | After Phase 5 done gates and identity/presence semantics are stable. |
| 4 | **Semantic Fisheye + DOI (focus+context without geometric distortion)** | High-value readability improvement for dense graphs; preserves mental map while surfacing relevance. | `2026-02-18_graph_ux_research_report.md` §§13.2, 14.8, 14.9 | After basic LOD and viewport culling are in place. |
| 5 | **Frame-affinity organizational behavior / Group-in-a-Box / Query-to-Zone** (legacy alias: Magnetic Zones) | Adds spatial organization as a first-class workflow, not just emergent physics. | `2026-02-24_layout_behaviors_plan.md` Phase 3 (expanded with persistence scope, interaction model, and implementation sequence), `2026-02-18_graph_ux_research_report.md` §13.1 | **Prerequisites now documented** in `layout_behaviors_plan.md` §3.0–3.5. Implementation blocked on: (1) layout injection hook (Phase 2), (2) Canonical/Divergent scope settlement. Trigger: when both blockers are resolved, execute implementation sequence in §3.5. |
| 6 | **Graph Reader ("Room" + "Map" linearization) and list-view fallback** | Critical accessibility concept beyond the initial webview bridge; gives non-visual users graph comprehension. | `2026-02-24_spatial_accessibility_research.md`, `SUBSYSTEM_ACCESSIBILITY.md` §8 Phase 2 | After Phase 1 WebView Bridge lands. |
| 7 | **Unified Omnibar (URL + graph search + web search heuristics)** | Core browser differentiator; unifies navigation and retrieval. | `GRAPHSHELL_AS_BROWSER.md` §7, `2026-02-18_graph_ux_research_report.md` §15.4 | After command palette/input routing stabilization. |
| 8 | **Progressive Lenses + Lens/Physics binding policy** | Makes Lens abstraction feel native and semantic, not static presets. | `2026-02-24_interaction_and_semantic_design_schemes.md`, `2026-02-24_physics_engine_extensibility_plan.md` (lens-physics binding preference) | After Lens resolution is active runtime path and physics presets are distinct in behavior. |
| 9 | **2D↔3D Hotswitch with `ViewDimension` and position parity** | Named first-class vision feature; fits the new per-view architecture and future Rapier/3D work. | `2026-02-24_physics_engine_extensibility_plan.md`, `design_docs/PROJECT_DESCRIPTION.md` | After pane-hosted view model and `GraphViewState` are stable. |
| 10 | **Interactive HTML Export (self-contained graph artifact)** | Strong shareability and offline review workflow; distinctive output mode. | `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md` (archived concept) | After viewer/content model and export-safe snapshot shape are defined. |
```

### Original §2 Appended Adoption Notes (trailing blocks, now removed)

```markdown
Appended adoption note (preserved from PR `#55`, pending table refactor):
- Ghost Nodes (`Rank 1`, formerly "Visual Tombstones") is now backed by `design_docs/graphshell_docs/implementation_strategy/2026-02-26_visual_tombstones_plan.md` and should be treated as `✅ adopted` in future table cleanup.

Appended adoption note (preserved from PR `#56`, pending table refactor):
- Temporal Navigation / Time-Travel Preview (`Rank 2`) should be treated as `✅ adopted` and promoted to a tracked staged backlog item via `design_docs/graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md` Stage F.

Appended staged backlog summary (preserved from PR `#56`, pending section refactor):
- **Stage F: Temporal Navigation (Tracked Staged Backlog Item)** — Deferred until Stage E History Manager maturity (tiered storage, dissolution correctness, and stable WAL shape).
- Deliverables preserved from PR summary: timeline index, `replay_to_timestamp(...)`, detached preview graph state, timeline slider/return-to-present UI, and preview ghost rendering.
- Preview-mode effect isolation contract (preserved): no WAL writes, no webview lifecycle mutations, no live graph mutations, no persistence side effects, and clean return-to-present with no preview-state leakage.
- Designated enforcement point preserved: `desktop/gui_frame.rs` effect-suppression gates.
- Preserved non-goals: collaborative replay, undo/redo replacement, scrubber polish fidelity, timeline snapshot export.

Appended adoption note (preserved from PR `#58`, pending table refactor):
- Semantic Fisheye + DOI (`Rank 4`) is now backed by `design_docs/graphshell_docs/implementation_strategy/2026-02-25_doi_fisheye_plan.md` and should be linked from the forgotten-concepts table during later cleanup.

Appended adoption note (preserved from PR `#60`, pending table refactor):
- Progressive Lenses + Lens/Physics Binding Policy (`Rank 8`) now has a strategy doc: `design_docs/graphshell_docs/implementation_strategy/2026-02-25_progressive_lens_and_physics_binding_plan.md`; treat the concept as policy-specified (implementation still blocked on runtime prerequisites).

Appended adoption note (preserved from PR `#54`, pending table refactor):
- Collaborative Presence (`Rank 3`) is now backed by `design_docs/verse_docs/implementation_strategy/2026-02-25_verse_presence_plan.md` and should be linked from the forgotten-concepts table during later cleanup.
```
