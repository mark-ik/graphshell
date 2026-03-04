# Accessibility Closure Bundle Audit (`#301`)

**Date**: 2026-03-02  
**Status**: Closure evidence artifact  
**Scope**: `G-A-7`, `G-A-8`, `G-A-9`, `G-A-11` from `../research/2026-03-02_ux_integration_research.md`.

**Related**:
- `../design/accessibility_baseline_checklist.md`
- `../implementation_strategy/subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md`
- `../implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md`

---

## 1. Summary outcome

| Gap | Category | Outcome | Evidence type |
|---|---|---|---|
| `G-A-8` | Reduced motion | **Staged with guardrails** | Canonical policy + repeatable validation steps |
| `G-A-7` | Contrast | **Audited (key explicit-color command surfaces)** | Repeatable ratio calculation procedure + recorded outputs |
| `G-A-9` | Target size | **Audited (key explicit-size command surfaces)** | Constant-based size verification + documented exceptions |
| `G-A-11` | Keyboard trap | **Validated** | Deterministic focus-cycle/return-path tests |

---

## 2. Reduced-motion closure (`G-A-8`)

### 2.1 Implementation state

Preference-plumbed reduced-motion behavior is **staged** (not yet fully preference-driven).

### 2.2 Guardrails (required until full preference wiring)

1. Motion-critical behavior remains under explicit command authority (`GraphTogglePhysics`) and does not silently switch command semantics.
2. Focus and return-path transitions remain deterministic and non-animated by contract (focus/workbench specs), preserving non-pointer accessibility behavior.
3. Reduced-motion remains tracked as a first-class closure lane in canonical accessibility artifacts until preference plumbing is completed.

### 2.3 Repeatable validation step

- Verify deterministic physics control path remains explicit and stable through command surfaces (`ActionRegistry` path for `GraphTogglePhysics`) and does not introduce hidden motion-only routing.

---

## 3. Contrast audit artifact (`G-A-7`)

### 3.1 Procedure (repeatable)

Run this PowerShell snippet from repository root to compute WCAG contrast ratios for explicit-color radial command-surface pairs:

```powershell
function Convert-ToLinear([double]$c){ $v=$c/255.0; if($v -le 0.04045){ return $v/12.92 } else { return [math]::Pow((($v+0.055)/1.055),2.4) } }
function Get-Luminance($rgb){ return 0.2126*(Convert-ToLinear $rgb[0]) + 0.7152*(Convert-ToLinear $rgb[1]) + 0.0722*(Convert-ToLinear $rgb[2]) }
function Get-Contrast($fg,$bg){ $l1=Get-Luminance $fg; $l2=Get-Luminance $bg; if($l2 -gt $l1){ $tmp=$l1; $l1=$l2; $l2=$tmp }; return ($l1+0.05)/($l2+0.05) }
```

### 3.2 Recorded outputs

| Pair | Ratio | WCAG 1.4.3 text threshold (4.5:1) | Status |
|---|---:|---:|---|
| radial enabled text on enabled button | 6.99 | 4.5 | Pass |
| radial disabled text on disabled button (2026-03-02 baseline) | 3.21 | 4.5 | Fail (text); Pass for non-text minimum 3.0 |
| radial hub label on hub fill | 12.78 | 4.5 | Pass |
| radial domain label on domain fill | 10.34 | 4.5 | Pass |
| radial hover-label text on hover-label background | 14.22 | 4.5 | Pass |
| radial page-indicator text on canvas background | 8.55 | 4.5 | Pass |

### 3.4 Remediation addendum (2026-03-04)

- Disabled radial command text color was remediated in `render/radial_menu.rs` and is now gated by an automated contrast regression test (`radial_disabled_text_contrast_meets_wcag_minimum_for_text`).
- Post-fix measured ratio for disabled text on disabled button is approximately `6.05:1` (>= `4.5:1`) and therefore now passes WCAG 1.4.3 normal text threshold.

### 3.3 Exception log

- Historical note: disabled-state text contrast in radial surface was previously `3.21:1`; this was remediated on 2026-03-04 (see §3.4).

---

## 4. Target-size audit artifact (`G-A-9`)

### 4.1 Procedure (repeatable)

Verify explicit control-size constants in `render/radial_menu.rs`:

- `COMMAND_BUTTON_RADIUS = 22.0` → diameter `44px`
- WCAG 2.5.8 minimum target size: `24px`

### 4.2 Recorded outcomes

| Surface/control | Measured size | Threshold | Status |
|---|---:|---:|---|
| Radial command button | 44px diameter | 24px | Pass |
| Radial domain button | 52px diameter (`radius=26`) | 24px | Pass |

### 4.3 Documented exceptions

- Toolbar/menu controls without explicit size constants remain pending measurement in a follow-on target-size sweep.

---

## 5. Keyboard-trap mitigation validation (`G-A-11`)

### 5.1 Repeatable validation commands

Run:

- `cargo test cycle_focus_region_intent_cycles_graph_node_tool_regions -- --nocapture`
- `cargo test close_settings_tool_pane_restores_previous_graph_focus_via_orchestration -- --nocapture`
- `cargo test cycle_focus_region_success_does_not_emit_ux_navigation_violation_channel -- --nocapture`

### 5.2 Recorded result

All listed tests pass, validating deterministic non-pointer escape/return paths and no-trap navigation behavior in host UI focus routing.

### 5.3 Modal surface addendum (2026-03-04)

Additional regression coverage now verifies global undo shortcut modal isolation across multiple floating command surfaces:

- `global_shortcut_undo_is_consumed_when_modal_is_active` (radial)
- `global_shortcut_undo_is_consumed_when_command_palette_modal_is_active`
- `global_shortcut_undo_is_consumed_when_help_panel_modal_is_active`
- `global_shortcut_undo_is_consumed_when_clear_confirm_modal_is_active` (clear-data confirm dialog path)

This path now routes through a unified modal-surface dispatch gate shared with UI overlay state, so clear-confirm dialog activation participates in the same no-trap shortcut isolation contract as command overlays.

All pass and provide evidence that active modal overlays consume non-modal global shortcut handling instead of trapping focus/dispatch in ambiguous paths.

### 5.4 Text-input capture addendum (2026-03-04)

Input-layer shortcut capture behavior is now explicitly covered with tests in `input/mod.rs`:

- `collect_actions_suppresses_f6_when_keyboard_is_captured`
- `collect_actions_allows_f6_when_keyboard_is_not_captured`
- `collect_actions_keeps_f9_global_when_keyboard_is_captured`
- `collect_actions_suppresses_f1_when_keyboard_is_captured`
- `collect_actions_suppresses_f2_when_keyboard_is_captured`
- `collect_actions_suppresses_f3_when_keyboard_is_captured`

This validates that focus-cycling shortcuts are suppressed while text-edit surfaces own keyboard focus, while designated global safety/escape controls remain available.

### 5.5 Workbar keyboard-operability addendum (2026-03-04)

Keyboard operability coverage now explicitly includes workbar command-surface toggles through input-layer key-detection tests:

- `collect_actions_maps_f1_to_help_panel_toggle_when_not_captured`
- `collect_actions_maps_f2_to_command_palette_toggle_when_not_captured`
- `collect_actions_maps_f3_to_radial_menu_toggle_when_not_captured`

Together with existing intent-application tests (`test_toggle_help_panel_action`, `test_toggle_command_palette_action`), this provides direct evidence that core workbar command surfaces are keyboard-invokable.

### 5.6 Omnibar keyboard-submit addendum (2026-03-04)

Omnibar submit-dispatch behavior now has explicit helper-level coverage in `toolbar_location_panel.rs`:

- `submit_dispatch_triggers_for_focused_enter`
- `submit_dispatch_triggers_for_queued_submit`
- `submit_dispatch_ignores_enter_after_focus_loss`
- `submit_dispatch_triggers_for_queued_submit_after_focus_loss`
- `submit_dispatch_does_not_trigger_without_enter_or_queue`

This confirms keyboard-submit semantics remain deterministic (focused Enter and queued-submit paths dispatch; passive Enter after focus loss does not), reducing no-trap/accidental-submit ambiguity for omnibar workflows.

### 5.7 Dialog destructive-confirmation addendum (2026-03-04)

Clear-data destructive confirmation timing now has explicit unit coverage in `dialog_panels.rs`:

- `clear_data_confirm_is_not_armed_without_deadline`
- `clear_data_confirm_is_armed_until_deadline_inclusive`
- `clear_data_confirm_expires_after_deadline_passes`
- `next_clear_data_confirm_deadline_uses_expected_window`

This validates deterministic two-step confirmation semantics (arm on first activation, execute only while armed, require re-arm after expiry), which contributes direct error-prevention evidence for destructive dialog actions.

### 5.8 Labels/Instructions baseline addendum (2026-03-04)

Instructional copy invariants now have explicit regression coverage:

- `clear_data_confirm_warning_text_includes_instruction_and_timing`
- `clear_data_confirm_success_text_describes_completed_action`
- `location_input_hint_text_provides_search_and_address_instruction`

These tests preserve baseline instruction clarity for destructive dialogs and the omnibar primary text-entry affordance, providing concrete partial evidence for WCAG 3.3.2 mapping while broader multi-surface validation remains pending.

### 5.9 Character-key shortcut capture addendum (2026-03-04)

Input-layer shortcut suppression under text capture now explicitly includes character-key shortcuts:

- `collect_actions_suppresses_character_shortcut_n_when_keyboard_is_captured`
- `collect_actions_suppresses_character_shortcut_t_when_keyboard_is_captured`
- `collect_actions_suppresses_character_shortcut_questionmark_when_keyboard_is_captured`

This extends keyboard-capture evidence beyond function keys, showing that single-character command shortcuts are not triggered while text-entry controls own keyboard focus.

### 5.10 On-Input deterministic side-effects addendum (2026-03-04)

Dialog clear-confirm input processing now has explicit action-classification regressions:

- `clear_data_confirm_action_arms_when_no_deadline_is_present`
- `clear_data_confirm_action_executes_when_deadline_is_active`
- `clear_data_confirm_action_rearms_when_deadline_expired`

Together with existing omnibar submit-dispatch tests (`submit_dispatch_*`), this provides concrete evidence that input-side effects are deterministic and state-dependent rather than surprising implicit context changes.

### 5.11 Sensory-independent disabled-state guidance addendum (2026-03-04)

Command-surface disabled precondition guidance now has explicit regressions:

- `all_disabled_actions_expose_textual_precondition_reason_in_default_context`
- `disabled_action_reasons_use_actionable_text_not_color_cues`

These tests provide partial evidence that disabled-state communication is conveyed via textual/actionable guidance rather than relying on color-only cues, supporting WCAG 1.3.3 and 1.4.1 mapping in command surfaces.

### 5.12 Link-purpose label clarity addendum (2026-03-04)

Action label purpose clarity now has targeted regression coverage in `action_registry.rs`:

- `test_representative_action_labels_convey_purpose_in_context`

The test verifies representative Open/Copy/Connect/Delete/Save/Restore command labels contain explicit purpose terms, providing partial evidence for WCAG 2.4.4 mapping on command surfaces.

---

## 6. Done-gate mapping (`#301`)

- [x] Reduced-motion behavior implemented **or explicitly staged with guardrails**.
- [x] Contrast/target-size audit artifacts committed.
- [x] Keyboard trap mitigation validated.
