# Accessibility Baseline Checklist

**Date**: 2026-03-02  
**Status**: Canonical deliverable (D4)  
**Purpose**: WCAG 2.2 Level A + AA baseline checklist across major Graphshell surfaces.

**Related**:
- `../research/2026-03-02_ux_integration_research.md`
- `../implementation_strategy/subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md`
- `../implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md`
- `../implementation_strategy/subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../testing/2026-03-02_accessibility_closure_bundle_audit_301.md`
- `../testing/2026-03-02_graph_canvas_keyboard_focus_audit_298.md`

---

## 1. Scope

Surface columns in this checklist:

- Graph Pane
- Node Pane
- Tool Pane
- Floating Windows
- Dialogs
- Omnibar
- Workbar

Status vocabulary:

- **Pass**
- **Fail**
- **N/A**
- **Untested**

Known-gap alignment from UX integration research:

- Graph nodes keyboard focusability gap (`G-A-1`)
- Graph node accessible-name gap (`G-A-3`)
- Focus-order and predictability risks (`G-PS-*`)

---

## 2. WCAG 2.2 Level A + AA checklist

| Criterion | Level | Graph Pane | Node Pane | Tool Pane | Floating Windows | Dialogs | Omnibar | Workbar | Notes |
|---|---|---|---|---|---|---|---|---|---|
| 1.1.1 Non-text Content | A | Fail | Untested | Untested | Untested | Untested | Untested | Untested | Graph nodes/icons need verified text alternatives and accessible names. |
| 1.2.1 Audio-only and Video-only (Prerecorded) | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No dedicated prerecorded media workflow is currently scoped. |
| 1.2.2 Captions (Prerecorded) | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No prerecorded synchronized media surface in current UX baseline. |
| 1.2.3 Audio Description or Media Alternative (Prerecorded) | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | Not applicable without prerecorded synchronized media content path. |
| 1.2.4 Captions (Live) | AA | N/A | N/A | N/A | N/A | N/A | N/A | N/A | Live media/caption channel not in current baseline surface set. |
| 1.2.5 Audio Description (Prerecorded) | AA | N/A | N/A | N/A | N/A | N/A | N/A | N/A | Not applicable without prerecorded media path. |
| 1.3.1 Info and Relationships | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Requires semantic structure verification across panes and chrome. |
| 1.3.2 Meaningful Sequence | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Focus/reading order requires test confirmation per surface. |
| 1.3.3 Sensory Characteristics | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Verify instructions do not depend only on position/color/shape cues. |
| 1.3.4 Orientation | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Orientation constraints unverified for responsive/workbench layouts. |
| 1.3.5 Identify Input Purpose | AA | N/A | Untested | Untested | Untested | Untested | Untested | N/A | Primarily applies to input fields (omnibar/settings/dialog controls). |
| 1.4.1 Use of Color | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Contrast-independent state cues need explicit verification. |
| 1.4.2 Audio Control | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No auto-playing audio controls in current baseline. |
| 1.4.3 Contrast (Minimum) | AA | Untested | Untested | Untested | Pass | Untested | Untested | Untested | Floating command surface (radial menu) contrast audited and remediated (`#301` + 2026-03-04 follow-up); remaining surfaces still require explicit audit evidence. |
| 1.4.4 Resize Text | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Text scaling behavior requires dedicated verification sweep. |
| 1.4.5 Images of Text | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Confirm text remains real text for command/UI labels. |
| 1.4.10 Reflow | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Surface behavior spec defines overflow policy; implementation needs audit. |
| 1.4.11 Non-text Contrast | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Visual indicators/focus rings require contrast validation. |
| 1.4.12 Text Spacing | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Custom spacing tolerance not yet verified. |
| 1.4.13 Content on Hover or Focus | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Tooltip/overlay persistence and dismissibility needs testing. |
| 2.1.1 Keyboard | A | Pass | Untested | Untested | Untested | Untested | Untested | Pass | Graph-pane baseline includes deterministic keyboard traversal (`#298`), and workbar command-surface keyboard invocations (`F1`/`F2`/`F3`) now have explicit test evidence (2026-03-04 addendum). |
| 2.1.2 No Keyboard Trap | A | Pass | Untested | Untested | Pass | Untested | Untested | Untested | Host focus-cycle return-path validation is complete for graph-pane routing (`#301`), floating command overlays (radial/command palette/help panel) have explicit modal-isolation shortcut-consumption regressions, and input-layer capture tests now verify text-input shortcut suppression/allow-list behavior (2026-03-04 addendum). |
| 2.1.4 Character Key Shortcuts | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Single-key command behavior needs remap/disable verification. |
| 2.2.1 Timing Adjustable | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No time-limited interaction currently declared for these surfaces. |
| 2.2.2 Pause, Stop, Hide | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No auto-updating moving content contract in baseline surfaces. |
| 2.3.1 Three Flashes or Below Threshold | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Motion/flash safety not yet audited in rendering effects. |
| 2.4.1 Bypass Blocks | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Region cycling/skip semantics exist but need conformance verification. |
| 2.4.2 Page Titled | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | Desktop app surface; criterion mapped as not directly page-scoped. |
| 2.4.3 Focus Order | A | Pass | Untested | Untested | Untested | Untested | Untested | Untested | Graph-pane traversal order is now explicit and deterministic (`#298`). |
| 2.4.4 Link Purpose (In Context) | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Verify actionable labels communicate purpose clearly. |
| 2.4.5 Multiple Ways | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Requires evidence for alternate navigation/find paths. |
| 2.4.6 Headings and Labels | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Label clarity work intersects IA closure (`#299`). |
| 2.4.7 Focus Visible | AA | Fail | Untested | Untested | Untested | Untested | Untested | Untested | Graph focus indication needs full keyboard-visibility closure. |
| 2.4.11 Focus Not Obscured (Minimum) | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Overlay/popups may obscure focused controls; verify per surface. |
| 2.5.1 Pointer Gestures | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Multi-point/gesture alternatives not fully verified. |
| 2.5.2 Pointer Cancellation | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Drag/select/click cancellation semantics require audit evidence. |
| 2.5.3 Label in Name | A | Pass | Untested | Untested | Untested | Untested | Untested | Untested | Graph canvas accessibility label now reflects focused node naming (`#298`). |
| 2.5.4 Motion Actuation | A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No motion-actuation controls are baseline-required. |
| 2.5.7 Dragging Movements | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Keyboard/non-drag alternatives for drag interactions require validation. |
| 2.5.8 Target Size (Minimum) | AA | Untested | Untested | Untested | Pass | Untested | Untested | Untested | Floating command surface controls with explicit constants are audited (radial command + domain buttons in `#301`); other surfaces remain pending measurement sweep. |
| 3.1.1 Language of Page | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Verify language metadata at host/UI accessibility tree layer. |
| 3.1.2 Language of Parts | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Mixed-language content handling not yet audited. |
| 3.2.1 On Focus | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Focus should not trigger unexpected context changes; verify. |
| 3.2.2 On Input | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Input-change side effects need confirmation across settings/filters. |
| 3.2.3 Consistent Navigation | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Navigation order consistency requires dedicated audit pass. |
| 3.2.4 Consistent Identification | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Same controls need stable naming across panes/surfaces. |
| 3.2.6 Consistent Help | AA | N/A | N/A | N/A | N/A | N/A | N/A | N/A | No persistent help mechanism is baseline-required yet. |
| 3.3.1 Error Identification | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Error identification policy exists; runtime conformance pending audit. |
| 3.3.2 Labels or Instructions | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Verify forms/inputs provide explicit labels and instructions. |
| 3.3.3 Error Suggestion | A | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Recovery suggestion quality varies; requires per-surface tests. |
| 3.3.4 Error Prevention (Legal, Financial, Data) | AA | N/A | N/A | N/A | N/A | N/A | N/A | N/A | Baseline UX does not currently include legal/financial irreversible forms. |
| 3.3.6 Error Prevention (All) | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Destructive-action confirmation behavior is not uniformly implemented (`G-FR-2`). |
| 4.1.2 Name, Role, Value | A | Pass | Untested | Untested | Untested | Untested | Untested | Untested | Graph-pane baseline naming model is implemented and verified in `#298` audit evidence. |
| 4.1.3 Status Messages | AA | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Status/error/loading announcements need AT verification. |

---

## 3. Initial screen reader test matrix

This matrix seeds the required baseline for `#295` and follow-on implementation issues.

| Environment | Screen Reader | Graph Pane | Node Pane | Tool Pane | Floating Windows | Dialogs | Omnibar | Workbar | Notes |
|---|---|---|---|---|---|---|---|---|---|
| Windows 11 | NVDA (latest) | Planned | Planned | Planned | Planned | Planned | Planned | Planned | Primary baseline target for desktop validation. |
| Windows 11 | Narrator | Planned | Planned | Planned | Planned | Planned | Planned | Planned | Secondary Microsoft-native verification pass. |
| Linux (future run) | Orca | Planned | Planned | Planned | Planned | Planned | Planned | Planned | Optional near-term; useful for cross-platform parity. |
| macOS (future run) | VoiceOver | Planned | Planned | Planned | Planned | Planned | Planned | Planned | Required when macOS UX parity sweep is scheduled. |

Execution note:

- First pass should prioritize known gap surfaces (Graph Pane, Workbar/focus transitions, command surfaces).
- Results should be copied back into §2 statuses and linked to follow-up issues (`#298`, `#301`).

---

## 4. Initial implementation checklist

- [x] D4 checklist file exists at `design_docs/graphshell_docs/design/accessibility_baseline_checklist.md`.
- [x] Contains one row per WCAG 2.2 Level A + AA criterion with no blank status cells.
- [x] Includes all required surface columns (graph, node, tool, floating, dialogs, omnibar, workbar).
- [x] Initial screen reader test matrix is included.

---

## 5. Status delta update (`#301`)

Delta source: `../testing/2026-03-02_accessibility_closure_bundle_audit_301.md`.

| Gap | Previous status | Updated status | Evidence |
|---|---|---|---|
| `G-A-8` Reduced-motion support | Missing | Staged with guardrails | §2 in audit artifact |
| `G-A-7` Contrast ratios | Unverified | Key explicit-color command surfaces audited | §3 in audit artifact |
| `G-A-9` Target-size minimums | Unverified | Key explicit-size command surfaces audited; exceptions logged | §4 in audit artifact |
| `G-A-11` Keyboard trap | Partially addressed | Host UI no-trap return-path validation complete | §5 in audit artifact |

Maintenance rule: any accessibility behavior change that affects WCAG mapping must update this checklist and the UX parity trackers in the same PR.

---

## 6. Status delta update (`#298`)

Delta source: `../testing/2026-03-02_graph_canvas_keyboard_focus_audit_298.md`.

| Gap | Previous status | Updated status | Evidence |
|---|---|---|---|
| `G-A-1` Graph nodes keyboard focusability | Missing | Baseline deterministic traversal implemented (`Tab` / `Shift+Tab`) | §1.1 + §2 in audit artifact |
| `G-A-3` Graph node accessible names | Missing | Baseline naming exposure added to graph canvas accessibility label | §1.2 + naming policy in audit artifact |
| `G-A-4` Focus order in graph canvas | Missing | Deterministic `NodeKey` traversal order with wrap behavior | §1.1 + unit tests in audit artifact |