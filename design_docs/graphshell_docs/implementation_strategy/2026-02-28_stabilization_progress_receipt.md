# 2026-02-28 Stabilization Progress Receipt (`lane:stabilization` / `#88`)

## Scope

This receipt captures stabilization slices landed on `main` after the prior partial-progress reference in `PLANNING_REGISTER.md` and records issue-facing evidence for camera/input/focus/lasso regressions.

## Landed slices (chronological)

- `001a121` — keyboard zoom commands now requeue when graph metadata is temporarily unavailable, preventing dropped zoom intent behavior.
- `004fd13` — lasso boundary hit-testing updated to center-inclusive semantics with regression coverage.
- `18c6ae9` — node-open focus handoff hardened with timeline tracing to reduce blank-first-frame activation races.
- `874e2a6` — stale focus hints cleared during pane-close handoff.
- `441aded` — active graph pane recovery after node-pane close handoff, plus deterministic scenario/unit coverage.
- `d8983c9` — focus activation defers unmapped primary target and applies mapped fallback while preserving retry hint.
- `4708e55` — deferred focus activation diagnostics receipts added.
- `f12e0cc` — regression coverage added for deferred focus activation diagnostics emission.
- `37350e7` — background click deselect now ignores frames where graph primary-click actions were already handled.
- `4f1011f` — unmapped semantic child-webview opens are surfaced via warn-level diagnostics instead of silent drop.
- `e755f48` — unmapped semantic child-webview opens are deferred/retried across frames via runtime queueing.
- `57a55e6` — semantic child-webview pane opens now route through `OpenNodeFrameRouted` intent flow (same frame-routing semantics as other node opens).
- `7782fd9` — orchestration scenario coverage added for deferred child-webview retry and routed pane-open on first mapped frame.
- `1a6e8a8` — compositor pass-contract diagnostics coverage added for overlay-without-content pass-order violation and overlay style/mode emission channels.
- `67a4ad9` — scenario-level healthy compositor overlay diagnostics assertion added (style/mode channels present, pass-order violation absent).
- `936073e` — background click deselect is now deterministic (plain click only), with modifier/radial-open regression coverage.
- `b6b931b` — compositor pass-contract invariants hardened: pass-order violations are now render-mode-aware (composited-only), native-overlay chrome diagnostics are regression-tested, and GL framebuffer binding restoration now honors captured state.
- `0b837b4` — compositor render-mode policy regressions expanded to cover embedded/placeholder pass-order non-violation paths and hover-overlay style mapping for composited/native-overlay modes.
- `b77fd1e` — background pan no longer blocks on `radial_open`, preventing sticky radial-menu state from disabling graph pan controls; policy regression added.
- `2c1f3e1` — lasso metadata state keying is now explicitly regression-covered for per-view scoping (`metadata_id`-derived lasso state IDs differ across views).
- `64cd66d` — tile rearrange overlay scheduling regressions added: focused composited tiles now have explicit Focus overlay-pass scheduling evidence, and hovered native-overlay tiles have explicit Hover overlay scheduling evidence (fallback policy path).
- `5c3a175` — compositor replay-forensics substrate landed for `#166`: bounded replay sample ring capture in the compositor adapter, replay sample channels registered in phase diagnostics, and snapshot export now includes `compositor_replay_samples` evidence.
- `82f3712` — differential composition contract landed for `#167`: composited content pass now skips unchanged tile signatures (webview + pixel rect), preserves overlay affordance scheduling, emits skip/composed/fallback-reason diagnostics channels, and records per-frame skip-rate basis points.
- `37f2ba8` — diagnostics/profiling hook slice for `#184`: diagnostics snapshot JSON now exports `compositor_differential` summary metrics (composed/skipped/fallback counts and skip-rate basis points), and Diagnostics Inspector `Compositor` tab surfaces the same differential metrics in a dedicated summary grid.
- `187cb2e` — optimization/degradation policy slice for `#184`: compositor now culls off-viewport tile content callbacks, enforces an explicit per-frame composited content budget with placeholder-mode degradation under GPU-pressure conditions, and emits culling/degradation diagnostics channels surfaced via diagnostics summary/export.
- `0a25448` — batching/resource-reuse observability slice for `#184`: compositor now emits context reuse hit/miss channels and per-frame overlay batch-size samples, with diagnostics summary/export surfacing reuse and batching metrics alongside degradation and skip-rate signals.
- `045c8ca` — replay forensics inspector completion slice for `#166`: Diagnostics Inspector `Compositor` tab now surfaces replay availability summary (sample count, violation count, latest sequence/node/duration), snapshot export feedback explicitly reports replay sample/violation totals with export path, and snapshot JSON shape includes `compositor_replay` summary metadata alongside replay artifacts.
- `4b5e324` — selection-contract hardening slice for `#185`: node select/deselect hit resolution now emits explicit ambiguous-hit diagnostics when event node index cannot be resolved to a Graphshell node key, preserving prior selection truth instead of drifting state; regression tests cover both ambiguous and valid-hit paths.
- `5f61430` — open-routing contract slice for `#186`: selected-node open execution now consumes queued open mode as a one-shot signal per action, pending open requests are routed through one deterministic node+mode path, split-mode node opens normalize leaf-root wrapping before split construction, and targeted regressions cover one-shot mode consumption, split-route focus handoff, and split root wrapping.

## Validation evidence

- Targeted workbench/unit tests for focus activation targeting and diagnostics emission are passing.
- Targeted replay-forensics tests are passing (`replay_ring_is_bounded_to_capacity`, `guarded_callback_with_snapshots_returns_before_and_after_states`, `diagnostics_json_snapshot_shape_is_stable`, `snapshot_json_includes_compositor_replay_samples_section`, `diagnostics_registry_declares_phase3_identity_channels_with_versions`).
- Targeted differential-composition tests are passing (`differential_content_decision_skips_when_signature_is_unchanged`, `differential_content_decision_recomposes_when_signature_changes`, `focus_overlay_scheduling_is_preserved_when_content_signature_is_clean`, `hover_overlay_scheduling_is_preserved_when_content_signature_is_clean`, `diagnostics_registry_declares_phase3_identity_channels_with_versions`).
- Targeted diagnostics snapshot tests for differential summary are passing (`diagnostics_json_snapshot_shape_is_stable`, `snapshot_json_includes_compositor_differential_summary_section`, `snapshot_json_includes_compositor_replay_samples_section`).
- Targeted optimization/degradation regressions are passing (`should_cull_tile_content_when_disjoint_from_viewport`, `should_not_cull_tile_content_when_visible_in_viewport`, `gpu_pressure_degradation_triggers_at_budget_boundary`, `diagnostics_registry_declares_phase3_identity_channels_with_versions`, `snapshot_json_includes_compositor_differential_summary_section`).
- Targeted batching/reuse diagnostics tests are passing (`diagnostics_registry_declares_phase3_identity_channels_with_versions`, `snapshot_json_includes_compositor_differential_summary_section`, `should_cull_tile_content_when_disjoint_from_viewport`, `gpu_pressure_degradation_triggers_at_budget_boundary`).
- Targeted replay inspector/export diagnostics tests are passing (`diagnostics_json_snapshot_shape_is_stable`, `replay_export_feedback_includes_path_and_counts`, `snapshot_json_includes_compositor_replay_samples_section`).
- Targeted selection-contract tests are passing (`test_select_node_action_ctrl_click_adds_to_selection`, `test_select_node_single_click_does_not_affect_multi_selection`, `test_node_key_or_emit_ambiguous_hit_emits_diagnostic_on_none`, `test_node_key_or_emit_ambiguous_hit_does_not_emit_for_valid_node`).
- Targeted open-routing contract tests are passing (`pending_open_mode_is_one_shot_after_execution`, `pending_open_request_split_mode_uses_split_route_and_focuses_node`, `open_or_focus_node_pane_split_wraps_leaf_root_before_split`).
- `cargo check` passed for each stabilization slice.
- `scripts/dev/smoke-matrix.ps1 quick` (Windows quick profile) passed after the latest stabilization test coverage commit.
- `scripts/dev/smoke-matrix.ps1 quick` (Windows quick profile) passed after each newly landed stabilization slice above.

## Register-state update intent

This lane remains `partial` in readiness terms because the stabilization bug register in `PLANNING_REGISTER.md` still includes unresolved items outside the landed focus/lasso/camera slices (notably compositor pass-contract closure evidence and remaining active repro verification).

## Issue update payload (`#88`)

Suggested issue comment body:

> Stabilization progress receipt (2026-02-28) has been landed in docs with commit evidence and validation notes:
> `design_docs/graphshell_docs/implementation_strategy/2026-02-28_stabilization_progress_receipt.md`.
>
> Newly landed commits on `main`: `001a121`, `004fd13`, `18c6ae9`, `874e2a6`, `441aded`, `d8983c9`, `4708e55`, `f12e0cc`, `37350e7`, `4f1011f`, `e755f48`, `57a55e6`, `7782fd9`, `1a6e8a8`, `67a4ad9`, `936073e`, `b6b931b`, `0b837b4`, `b77fd1e`, `2c1f3e1`, `64cd66d`.
>
> Net result: camera/zoom command reliability, lasso boundary correctness, click-away selection determinism, and compositor pass-contract diagnostics/state isolation robustness improved; pane-open/pane-close focus activation race is substantially hardened with regression evidence. Lane remains partial pending closure of remaining bug-register items and final compositor pass-contract closure evidence.
