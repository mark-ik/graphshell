# Current Milestone UX Contract Checklist

**Date**: 2026-02-28  
**Status**: Execution checklist  
**Purpose**: Define the highest-priority UX contract work needed to make Graphshell meaningfully usable as an application before deeper migration work.

**Primary references**:

- `2026-02-28_ux_contract_register.md`
- `2026-02-28_ux_issue_domain_map.md`
- `2026-02-27_ux_baseline_done_definition.md`
- `workbench/workbench_frame_tile_interaction_spec.md`
- `canvas/graph_node_edge_interaction_spec.md`
- `aspect_command/command_surface_interaction_spec.md`
- `subsystem_focus/focus_and_region_navigation_spec.md`
- `viewer/viewer_presentation_and_fallback_spec.md`
- `aspect_control/settings_and_control_surfaces_spec.md`
- `aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`

---

## 1. Goal

This checklist is not a full roadmap.

It is the short list of UX-contract work that most directly improves:

- app usability,
- ownership clarity,
- deterministic interaction,
- confidence in the current stack,

so that future migration work is grounded in a working application instead of a fragile prototype.

This checklist is the execution filter over the six canonical UX subsystem specs:

1. Workbench / Frame / Tile
2. Graph / Node / Edge
3. Command Surfaces
4. Focus and Region Navigation
5. Viewer Presentation and Fallback
6. Settings and Control Surfaces

---

## 2. Milestone Completion Standard

This milestone should be considered meaningfully complete when:

1. Core graph navigation is reliable.
2. Pane open/close/focus behavior is deterministic.
3. Content opening routes through Graphshell semantics.
4. Command surfaces do not diverge semantically.
5. Settings and history surfaces behave like real app surfaces.
6. Selection and fallback behavior are explicit and testable.

This is not “perfect UX.” It is “usable application baseline.”

---

## 3. Priority Checklist

### A. Navigation and Camera

- [ ] `#173` Graph camera controls work reliably in active graph panes
- [ ] `#104` Zoom-to-fit is restored and regression-tested
- [ ] `#101` Camera commands route to the correct `GraphViewId`
- [ ] `#103` Input ownership is audited so graph interactions are not lost to hidden/active pane ambiguity

Why this is first:

- If the user cannot reliably move through the graph, the rest of the app is difficult to evaluate.
- This is the first execution slice of the graph interaction spec.

### B. Pane and Workbench Lifecycle

- [ ] `#174` New pane spawn/focus/first-render activation is deterministic
- [ ] `#186` Opening the current selection into pane/tab/split follows one deterministic contract
- [ ] `#187` Closing the active pane returns focus to a valid visible next context
- [ ] `#118` `gui.rs` responsibility split reduces orchestration ambiguity where needed
- [ ] `#119` `gui_frame.rs` responsibility split reduces frame lifecycle ambiguity where needed

Why this is second:

- This is the backbone of “the app feels like an application” rather than a fragile prototype.
- This is the first execution slice of the workbench and focus specs.

### C. Content Opening and Routing

- [ ] `#175` Content-originating open flows route through Graphshell node/tile semantics

Why this matters:

- If web content can bypass Graphshell semantics, app authority is not real.
- This is shared boundary work across the graph and workbench specs.

### D. Command Surface Unification

- [ ] `#176` Command semantics unify across keyboard and context surfaces
- [ ] `#108` One command dispatch boundary exists for major command surfaces
- [ ] `#106` Global trigger parity exists for the command palette
- [ ] `#107` Command palette semantics are clarified

Why this matters:

- Command surfaces are how a lot of advanced UX gets expressed. If they diverge, the app becomes cognitively inconsistent.
- This is the first execution slice of the command-surface spec.

### E. Settings and Control Surfaces

- [ ] `#109` Settings opens as a real page-backed scaffold
- [ ] `#110` Settings has an explicit IA skeleton
- [ ] `#177` Theme mode toggle exists in the settings model
- [ ] `#189` Settings/history open and exit preserve user context
- [ ] `#178` Omnibar search iteration preserves input focus

Why this matters:

- The app needs real control surfaces, not placeholders and incidental flows.
- This is the first execution slice of the settings/control spec.

### F. Selection and Viewer Clarity

- [ ] `#185` Single-select and additive-select use an app-owned contract
- [ ] `#102` Lasso selection behavior is corrected on the current stack
- [ ] `#162` Overlay affordance policy is explicit by render mode
- [ ] `#188` Fallback and degraded viewer states are explicit and reasoned

Why this matters:

- Selection and visibility are core to user trust in the interface.
- This is the first execution slice of the graph and viewer specs.

---

## 4. Execution Order Recommendation

Recommended order:

1. **Camera and input control first**
   - `#173`, `#104`, `#101`, `#103`
2. **Pane lifecycle and focus next**
   - `#174`, `#186`, `#187`
3. **Route all content through Graphshell semantics**
   - `#175`
4. **Unify command semantics**
   - `#176`, `#108`, `#106`, `#107`
5. **Finish control surfaces**
   - `#109`, `#110`, `#177`, `#178`, `#189`
6. **Tighten selection and viewer-state clarity**
   - `#185`, `#102`, `#162`, `#188`
7. **Use targeted responsibility-split work only where it unlocks the above**
   - `#118`, `#119`

This order prioritizes immediate usability over broad cleanup.

It also walks the six-spec family in the right dependency order:

1. graph basics,
2. workbench and focus stability,
3. routing authority,
4. command unification,
5. control-surface legitimacy,
6. viewer clarity.

---

## 5. Out of Scope for This Milestone

The following remain important, but are not required for this UX-contract milestone:

- backend migration to `egui_wgpu`
- custom canvas replacement work (`#179`-`#184`)
- deep renderer optimization
- broad diagnostics platform expansion beyond what current UX issues require
- speculative 2.5D / 3D work
- replacing `egui_tiles`

Those can follow once the app is usable on the current stack.

---

## 6. Milestone Exit Questions

Before declaring this milestone complete, ask:

1. Can a user navigate the graph without fighting the app?
2. Can a user open, move through, and close panes predictably?
3. Do content-opening flows respect Graphshell authority?
4. Do commands mean the same thing across surfaces?
5. Are settings/history real surfaces with reliable return paths?
6. Are selection and degraded rendering states explicit enough to trust?

If the answer to any of these is “not really,” the milestone is not done.


