# Planning Register

**Status**: Active / Canonical (consolidated 2026-02-25)
**Purpose**: Single source for execution priorities, issue-ready backlog stubs, and implementation guidance.

## Contents

1. Immediate Priorities Register (10/10/10)
2. Latest Checkpoint Delta (Code + Doc Audit)
3. Merge-Safe Lane Execution Reference (Canonical)
4. Register Size Guardrails + Archive Receipts
5. Top 10 Active Execution Lanes (Strategic / Completion-Oriented)
6. Prospective Lane Catalog (Comprehensive)
7. Forgotten Concepts for Adoption (Vision / Research)
8. Quickest Improvements (Low-Effort / High-Leverage)
9. Historical Execution Sequence + Registry Closure Backlog (Reference)
10. Backlog Ticket Stubs (Index)
11. Implementation Guides (Index)
12. Suggested Tracker Labels
13. Import Notes

### Contents Notes

- `§1A` is the canonical sequencing control-plane section.
- `§1C` is the current prioritized lane board.
- `§1D` is the comprehensive lane catalog (including prospective and incubation lanes).
- Later duplicated numeric section labels (`## 2`..`## 5` repeated near the end of the file) are retained for archive/reference continuity and should be treated as reference payload, not canonical sequencing state.

---

## 1. Immediate Priorities Register (10/10/10)

_Source file before consolidation: `2026-02-24_immediate_priorities.md`_


**Status**: Active / Execution (revised 2026-02-25)
**Context**: Consolidated execution register synthesized from current implementation strategy, research, architecture, and roadmap docs.

**Audit basis (2026-02-25 review)**:
- `2026-02-22_registry_layer_plan.md`
- `2026-02-22_multi_graph_pane_plan.md` (scope expanded in paired doc sync to pane-hosted multi-view architecture)
- `2026-02-24_layout_behaviors_plan.md`
- `2026-02-24_performance_tuning_plan.md`
- `2026-02-24_control_ui_ux_plan.md`
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-24_spatial_accessibility_plan.md` → superseded by `SUBSYSTEM_ACCESSIBILITY.md`
- `2026-02-24_universal_content_model_plan.md`
- `2026-02-23_wry_integration_strategy.md`
- `2026-02-20_edge_traversal_impl_plan.md`
- `2026-02-18_graph_ux_research_report.md`
- `2026-02-24_interaction_and_semantic_design_schemes.md`
- `2026-02-24_diagnostics_research.md`
- `2026-02-24_visual_tombstones_research.md`
- `2026-02-24_spatial_accessibility_research.md`
- `GRAPHSHELL_AS_BROWSER.md`
- `IMPLEMENTATION_ROADMAP.md`
- `design_docs/PROJECT_DESCRIPTION.md`

---

## 0. Latest Checkpoint Delta (Code + Doc Audit)

### Code checkpoint (2026-02-24)

- Registry Phase 6.2 boundary hardening advanced: workspace-only reducer path extracted and covered by boundary tests.
- Registry Phase 6.3 single-write-path slices closed for runtime/persistence: direct persistence topology writes were converged to graph-owned helpers, runtime contract coverage now includes persistence runtime sections, and targeted boundary tests are green.
- Registry Phase 6.4 started with a mechanical host subtree move: `running_app_state.rs` and `window.rs` are now canonical under `shell/desktop/host/` with root re-export shims retained during transition.
- Registry Phase 6.4 import canonicalization advanced beyond `shell/desktop/**`: remaining root-shim host imports in `egl/app.rs` and `webdriver.rs` were moved to canonical `shell/desktop/host/*` paths; shim files remain in place for transition compatibility.
- Phase 5 sync UI/action path advanced: pair-by-code decode, async discovery enqueue path, and Phase 5 diagnostics channel + invariant contracts are now in code with passing targeted tests.
- Compile baseline remains green (`cargo check`), warning baseline unchanged.

### Doc audit delta (2026-02-25)

- Immediate-priority list promoted from a loose synthesis into a source-linked 10/10/10 register.
- Multi-pane planning is now treated as a **pane-hosted multi-view problem** (graph + viewer + tool panes), not only "multi-graph."
- Several low-effort, high-impact items from UX and diagnostics research were missing from the active queue and are now explicitly tracked.
- **Cross-cutting subsystem consolidation**: Five runtime subsystems formalized with dedicated subsystem guides:
  - `SUBSYSTEM_ACCESSIBILITY.md` — consolidates prior archived accessibility planning/detail docs in `design_docs/archive_docs/checkpoint_2026-02-25/` (both now superseded)
  - `SUBSYSTEM_DIAGNOSTICS.md` — elevated from `2026-02-24_diagnostics_research.md`
  - `SUBSYSTEM_SECURITY.md` — new; consolidates security/trust material from Verse Tier 1 plan + registry layer plan Phase 5.5
  - `SUBSYSTEM_STORAGE.md` — new; consolidates persistence material from registry layer plan Phase 6 + `services/persistence/mod.rs`
  - `SUBSYSTEM_HISTORY.md` — new; consolidates traversal/archive/replay integrity guarantees and Stage F temporal navigation constraints
- Surface Capability Declarations adopt the **folded approach** (sub-fields on `ViewerRegistry`, `CanvasRegistry`, `WorkbenchSurfaceRegistry` entries — not a standalone registry). See `TERMINOLOGY.md`.

### Subsystem Implementation Order (Current Priority)

This section sequences subsystem work by architectural leverage and unblock status. It links to subsystem guides instead of repeating subsystem contracts.

| Order | Subsystem | Why Now | Best Next Slice | Key Blockers / Dependencies |
| --- | --- | --- | --- | --- |
| 1 | `diagnostics` | Enables confidence and regression detection across all other subsystems. | Expand invariant coverage + pane health/violation views; continue severity-driven surfacing. | Pane UX cleanup; cross-subsystem channel adoption. |
| 2 | `storage` | Data integrity and persistence correctness are hard failure domains and a dependency for reliable history. | Add `persistence.*` diagnostics, round-trip/recovery coverage, degradation wiring. | App-level read-only UX wiring; crypto overlap with `security`. |
| 3 | `history` | Temporal replay/preview and traversal correctness depend on `storage` guarantees and become a core user-facing integrity concern. | Add `history.*` diagnostics + traversal/archive correctness tests before Stage F replay UI. | Stage E history maturity; persistence diagnostics/archives. |
| 4 | `security` | High-priority trust guarantees, but some slices are tied to Verse Phase 5.4/5.5 closure sequencing. | Grant matrix coverage + denial-path diagnostics assertions + trust-store integrity tests. | Verse sync path closure and shared `GraphIntent` classification patterns. |
| 5 | `accessibility` | Project goal and major concern, but Graph Reader breadth should follow the immediate WebView bridge fix and diagnostics scaffolding. | WebView bridge compatibility fix (`accesskit` alignment/conversion) + anchor mapping + bridge invariants/tests. | `accesskit` version mismatch; pane/view lifecycle anchor registration; view model stabilization for Graph Reader. |

---

## 1A. Merge-Safe Lane Execution Reference (Canonical)

This section is the canonical sequencing reference for conflict-aware execution planning, aligned with `CONTRIBUTING.md` lane rules (one active mergeable PR per lane when touching shared hotspots).

### Lane sequencing rules

- Use one active mergeable PR per lane for hotspot files (`app.rs`, `render/mod.rs`, workbench/gui integration paths).
- Use stacked PRs for dependent issue chains; merge bottom → top.
- Avoid cross-lane overlap on the same hotspot files within the same merge window.
- Treat this section as **active control-plane state**; treat detailed ticket stubs below as reference material.

### Recommended execution sequence (current)

Snapshot note (2026-02-26 queue execution audit + tracker reconciliation):
- The previously queued implementation chains below were audited and reconciled in issue state (closed):
  - `lane:p6`: `#76`, `#77`, `#63`-`#67`, `#79`
  - `lane:p7`: `#68`-`#71`, `#78`, `#80`, `#82`
  - `lane:p10`: `#74`, `#75`, `#73` and parent `#10`
  - `lane:runtime`: `#81`
  - `lane:quickwins`: `#21`, `#22`, `#27`, `#28`
  - `gap-remediation hub`: `#86`
- Evidence/receipt: `design_docs/archive_docs/checkpoint_2026-02-26/2026-02-26_planning_register_queue_execution_audit_receipt.md`

1. **lane:stabilization (ad hoc bugfix lane, active if user-reported regressions exist)**
  - Zoom-to-fit / unresponsive controls investigation (create/label issue before code work if no tracker exists)
  - Hub: `#88` (Controls/camera/focus correctness stabilization tracker)
  - Hotspots likely: `render/mod.rs`, `app.rs`, `gui.rs`, input/camera command paths
  - Rule: run as a single focused PR, do not overlap with quick refactors in the same hotspots
2. **lane:roadmap (docs/planning, merge-safe default lane)**
  - `#11` → `#12` → `#13` → `#14` → `#18` → `#19`
  - Low conflict risk with runtime/render hot files; preferred background lane while bugfix lane is idle
3. **lane:runtime-followon (new tickets required)**
  - `SYSTEM_REGISTER.md` SR2 (signal routing contract) before SR3 (`SignalBus`/equivalent fabric)
  - Hub: `#91` (SR2/SR3 signal routing contract + fabric tracker)
  - Create new child issues before execution; avoid reusing closed queue-cleanup issues (`#80/#81/#82/#86`)
  - Keep separate from stabilization lane if touching `gui.rs` or registry runtime hotspots

### Near-term PR stack plan (merge order)

- Completed (2026-02-26 audit/reconciliation): `lane:p6`, `lane:p7` phase-1, `lane:p10`, `lane:runtime`, `lane:quickwins` queues listed above
- Active merge-safe default stack: `lane:roadmap` docs/planning items (`#11` → `#12` → `#13` → `#14` → `#18` → `#19`)
- Conditional priority override: `lane:stabilization` bugfix PR (zoom/control regression) supersedes roadmap lane while active
- Parallel planning only (no code until ticketed): Register signal-routing roadmap slices (SR2/SR3)

### Stabilization Bug Register (Active)

Track active regressions here before they get folded into broader refactors. These are the only ad hoc slices allowed to preempt the default lane stack.

| Bug / Gap | Symptom | Likely Hotspots | Notes / Architectural Context | Done Gate |
| --- | --- | --- | --- | --- |
| Graph canvas camera controls fail globally | `pan drag`, `wheel zoom`, `zoom in/out/reset`, and `zoom-to-fit` fail in the default graph pane (not just multi-pane) | `render/mod.rs`, `app.rs`, `shell/desktop/ui/gui.rs`, `input/mod.rs` | Recent camera-targeting fixes landed, but user still reports global camera-control failure; likely input gating/consumption, metadata availability, or focus ownership debt. | Default graph pane supports pan + wheel zoom + zoom commands again; repro is closed with targeted tests/diagnostics. |
| Lasso metadata ID mismatch after multi-view | Selection/lasso behavior breaks or targets wrong graph metadata in multi-pane scenarios | `render/mod.rs` | Known hardcoded `egui_graphs_metadata_` path needs per-view metadata keying. | Lasso works across split graph panes; test covers second pane / non-default `GraphViewId`. |
| Tab/pane spawn focus activation race (blank viewport) | Newly opened tab/pane sometimes spawns visually blank until extra clicks/tab switches; graph pane can remain unfocused after pane deletion | `shell/desktop/ui/gui.rs`, `shell/desktop/ui/gui_frame.rs`, `shell/desktop/workbench/*`, `shell/desktop/lifecycle/webview_controller.rs` | Looks like focus ownership + render activation ordering debt (likely overlaps `lane:embedder-debt` servoshell-era host/frame assumptions). | New focused panes render on first activation consistently; pane-deletion focus handoff is deterministic and renders immediately. |
| Selection deselect click-away inconsistency | Node selection works, but clicking background to deselect is "funky" and may hide state-transition edge cases | `render/mod.rs`, `input/mod.rs`, selection state in `app.rs` | Likely local selection-state logic plus pane-focus interaction. | Deselect-on-background-click behavior is deterministic and covered by targeted selection tests. |
| Lasso boundary miss at selection edge | Lasso sometimes misses nodes at the edge of the box despite user expectation that center-in-box should count | `render/mod.rs`, `render/spatial_index.rs` | Correctness issue first; live lasso preview UX should be tracked separately under `lane:control-ui-settings`. | Lasso inclusion semantics are documented (center-inclusive minimum) and covered by edge-boundary tests. |
| Tile rearrange focus indicator hidden under document view | Blue focus ring does not render over document/web content while rearranging tile | `shell/desktop/workbench/tile_compositor.rs` | Servo/texture path is a z-order bug; Wry/overlay path needs a distinct affordance policy (cannot fake egui-over-OS overlay parity). | Servo focus affordance visible during rearrange; Wry path has explicit fallback affordance and documented limitation. |
| Legacy web content context menu / new-tab path bypasses node semantics | Right-click or ctrl-click link in webpage can use short legacy menu/path and may open tile/tab without creating mapped graph node | `shell/desktop/ui/gui.rs`, `shell/desktop/host/*`, `shell/desktop/lifecycle/webview_controller.rs`, `shell/desktop/workbench/tile_runtime.rs` | Graphshell command/pane semantics are bypassed by legacy webview path; cross-lane with `lane:embedder-debt` + `lane:control-ui-settings`. | Web content open-in-new-view flows route through Graphshell node/pane semantics or are explicitly bridged/deferred with limitations documented. |
| Command palette trigger parity + naming confusion | F2 summons `Edge Commands`; pointer/global trigger availability is context-biased and inconsistent | `render/mod.rs`, `render/command_palette.rs`, `input/mod.rs` | Keyboard trigger exists; command-surface model/naming/context policy lag behind plan. | Shared command-surface model backs F2 and contextual palette variants; naming reflects actual scope (not `Edge Commands` unless edge-specific). |

#### Known Rendering/Input Regressions (tracked under `lane:stabilization`)

- Global graph camera interaction failure remains active in user repro (`pan`, `wheel zoom`, `zoom commands`, `zoom-to-fit`) even after recent targeting fixes.
- Pane/tab focus activation and render timing are inconsistent (blank viewport until extra clicks/tab switches in some flows).
- Focus ring over composited web content remains a compositor pass/state-contract issue (Servo path), not an `egui` layer-count issue.
- Input consumption/focus ownership edge cases remain likely when graph pane and node pane coexist.
- Lasso correctness follow-ons remain: edge-boundary inclusion semantics and selection-state polish.

Use these as first-pass stabilization issue seeds when a dedicated issue does not yet exist.

#### Command Surface + Settings Parity Checklist (tracked under `lane:control-ui-settings`)

- Command palette must remain keyboard-triggerable and gain non-node pointer/global trigger parity (canvas, pane/workspace chrome, nodes/edges).
- F2/global command surface and right-click/contextual command surface should share one backend model while allowing different presentation sizes.
- `Edge Commands` labeling should be retired or narrowed to truly edge-specific UI.
- Contextual command categories should map to actionable entities (node/edge/tile/pane/workbench/canvas) with a clear disabled-state policy.
- Radial menu needs spacing/readability polish before primary-use promotion.
- Omnibar node-search iteration should retain input focus after Enter in search mode.
- Theme mode toggle (`System` / `Light` / `Dark`) should be added to settings and persisted.
- Settings IA must converge from transitional legacy booleans/bridge path to one page-backed settings surface.
- Settings tool pane must graduate from placeholder to scaffolded runtime surface.

Issue-ready intake stubs from the latest user report:
- `design_docs/graphshell_docs/implementation_strategy/2026-02-26_stabilization_control_ui_issue_stubs_from_user_report.md`

### Debt-Retirement Lanes (Current)

- `lane:embedder-debt` (servoshell inheritance retirement)
  - Hub: `#90` (Servoshell inheritance retirement tracker)
  - Scope: `gui.rs`/`gui_frame.rs` decomposition, `RunningAppState` coupling reduction, host/UI boundary cleanup, misleading servoshell-era naming/comments removal
  - Important child slice: composited webview callback pass contract + GL state isolation (`tile_compositor.rs`) to fix Servo-path overlay affordance failures that are not Wry/native-overlay limitations
  - Primary guide: `design_docs/graphshell_docs/implementation_strategy/2026-02-20_embedder_decomposition_plan.md`
  - Rule: pair mechanical moves with invariants/tests; avoid mixing with feature work in the same PR

### Incubation Lanes (Parallel / Non-blocking)

- `lane:verse-intelligence`
  - Hub: `#93` (Model slots + memory architecture implementation tracker)
  - Open a hub + child issue stack for the two design-ready plans (currently no implementation lane):
  - `design_docs/verse_docs/implementation_strategy/2026-02-26_model_slots_adapters_udc_personalization_plan.md`
  - `design_docs/verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
  - First executable slices should be schemas/contracts + storage/index scaffolds (not model training)

### Spec/Code Mismatch Register (Active)

| Mismatch | Current Reality | Owner Lane | Done Gate |
| --- | --- | --- | --- |
| `viewer:settings` selected but not embedded | Viewer resolution can select `viewer:settings`, but node-pane renderer still falls back to non-embedded placeholder for non-web viewers. | `lane:viewer-platform` (`#92`), `lane:control-ui-settings` (`#89`) | Settings viewer path is renderable without placeholder fallback in node/tool contexts. |
| Browser viewer table vs implemented viewer surfaces | Spec/docs describe broader viewer matrix than runtime embedded implementations currently expose. | `lane:viewer-platform` (`#92`), `lane:spec-code-parity` (`#99`) | Viewer table claims are either implemented or explicitly downgraded with phased status. |
| Wry strategy/spec vs runtime registration/dependency path | Wry integration strategy exists, but runtime feature/dependency/registration path remains partial/transitional. | `lane:viewer-platform` (`#92`), `lane:spec-code-parity` (`#99`) | `viewer:wry` foundation is feature-gated and runtime-wired, or spec is marked deferred with constraints. |

---

## 1B. Register Size Guardrails + Archive Receipts

This register is intentionally large; to keep it operational for agents and contributors, apply the following:

- Keep **active sequencing + merge/conflict guidance** in Sections `1`, `1A`, and `1B`.
- Treat detailed issue stubs and long guidance sections as **reference payloads**.
- When sequencing decisions change materially, write a timestamped archive receipt in:
  - `design_docs/archive_docs/checkpoint_2026-02-25/`
- Archive receipt naming convention:
  - `YYYY-MM-DD_planning_register_<topic>_receipt.md`
- Archive receipts should include:
  - date/time window
  - lane order
  - issue stack order
  - hotspot conflict assumptions
  - closure/update criteria

Current receipt for this sequencing snapshot:
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_planning_register_lane_sequence_receipt.md`
- `design_docs/archive_docs/checkpoint_2026-02-26/2026-02-26_planning_register_queue_execution_audit_receipt.md` (queue execution audit + landed-status verification + `#70` lifecycle policy patch)

---

## 1C. Top 10 Active Execution Lanes (Strategic / Completion-Oriented)

This supersedes the earlier registry-closure-heavy priority table. The queue audit closed most of those slices in code/issue state; the remaining project risk is now concentrated in stabilization, architectural follow-ons, subsystem hardening, and design-to-code execution.

| Rank | Lane | Why Now | Primary Scope (Next Tasks) | Primary Sources / Hotspots | Lane Done Gate |
| --- | --- | --- | --- | --- | --- |
| 1 | **`lane:stabilization` (`#88`)** | User-visible regressions block trust and currently prevent normal graph interaction, masking deeper architecture mistakes. | Restore graph camera controls (pan/wheel/zoom/fit), close tab/pane focus/render activation regressions, finish lasso correctness follow-ons, and keep focus-affordance/compositor regressions isolated with tests/diagnostics. | `render/mod.rs`, `app.rs`, `shell/desktop/ui/gui.rs`, `input/mod.rs`, `shell/desktop/workbench/tile_compositor.rs`, `shell/desktop/workbench/*`; `SUBSYSTEM_DIAGNOSTICS.md` | Repros are tracked, fixed, and covered by targeted tests/receipts; normal graph interaction works reliably in default and split-pane contexts. |
| 2 | **`lane:control-ui-settings` (`#89`)** | Control surfaces and settings IA are now clearly specified by user needs, but the runtime UI still exposes transitional/legacy command surfaces. | Unify F2 + contextual command surfaces, retire/rename `Edge Commands`, define contextual category/disabled-state policy, radial readability pass, omnibar focus retention, theme mode toggle, settings scaffold replacing placeholder pane. | `2026-02-24_control_ui_ux_plan.md`, `2026-02-20_settings_architecture_plan.md`, `render/command_palette.rs`, `render/mod.rs`, `shell/desktop/ui/toolbar/*`, `shell/desktop/workbench/tile_behavior.rs` | Command surfaces share one dispatch/context model across UI contexts; settings pane supports theme mode and is no longer placeholder-only for core settings paths. |
| 3 | **`lane:embedder-debt` (`#90`)** | Servoshell inheritance debt is the main source of host/UI focus/compositor friction and still leaks legacy behavior into user-facing flows. | Decompose `gui.rs`/`gui_frame.rs`, reduce `RunningAppState` coupling, narrow host/UI boundaries, fix legacy webview context-menu/new-tab bypass paths, retire misleading servoshell-era assumptions/comments. | `2026-02-20_embedder_decomposition_plan.md`, `shell/desktop/ui/gui.rs`, `shell/desktop/ui/gui_frame.rs`, `shell/desktop/host/*`, `shell/desktop/lifecycle/webview_controller.rs` | One stage of decomposition lands with tests/receipts; legacy webview path bypasses are either bridged or retired; hotspot surface area is reduced. |
| 4 | **`lane:runtime-followon` (`#91`)** | `SYSTEM_REGISTER.md` remaining gaps are now mostly SR2/SR3 signal routing contract/fabric + observability. | Open child issues for SR2/SR3; implement typed signal envelope/facade, routing diagnostics, misroute observability, fabric/backpressure policy. | `SYSTEM_REGISTER.md`, `TERMINOLOGY.md`, `shell/desktop/runtime/control_panel.rs`, `shell/desktop/runtime/registries/mod.rs` | SR2/SR3 child issues are landed or explicitly ticketed with done gates; signal routing boundary is testable and observable. |
| 5 | **`lane:viewer-platform` (`#92`)** | Viewer selection/capability scaffolding is ahead of actual embedded viewers; Wry remains design-only. | Replace non-web viewer placeholders (`settings`/`pdf`/`csv` first), implement Wry feature gate + manager/viewer foundation, align Verso manifest/spec claims. | `2026-02-24_universal_content_model_plan.md`, `2026-02-23_wry_integration_strategy.md`, `GRAPHSHELL_AS_BROWSER.md`, `mods/native/verso/mod.rs`, `Cargo.toml`, `shell/desktop/workbench/tile_behavior.rs` | At least one non-web native viewer is embedded; `viewer:wry` foundation exists behind feature gate or spec/docs are explicitly downgraded. |
| 6 | **`lane:accessibility` (`#95`)** | Accessibility is a project-level requirement; phase-1 bridge work exists but Graph Reader/Inspector paths remain incomplete. | Finish bridge diagnostics/health surfacing, implement Graph Reader scaffolds, replace Accessibility Inspector placeholder pane, add focus/nav regression tests. | `SUBSYSTEM_ACCESSIBILITY.md`, `shell/desktop/workbench/tile_behavior.rs`, `shell/desktop/ui/gui.rs` | Accessibility Inspector is functional, bridge invariants/tests are green, and Graph Reader phase entry point exists. |
| 7 | **`lane:diagnostics` (`#94`)** | Diagnostics remains the leverage multiplier for every other lane and still lacks analyzer/test harness execution surfaces. | Implement `AnalyzerRegistry` scaffold, in-pane `TestHarness`, expanded invariants, better violation/health views, orphan-channel surfacing. | `SUBSYSTEM_DIAGNOSTICS.md`, `shell/desktop/runtime/diagnostics/*`, diagnostics pane code paths | Analyzer/TestHarness scaffolds exist and can be run in-pane (feature-gated if needed). |
| 8 | **`lane:subsystem-hardening` (`#96`)** | Storage/history/security are documented but still missing closure slices that protect integrity and trust. | Add `persistence.*` / `history.*` / `security.identity.*` diagnostics, degradation wiring, traversal/archive correctness tests, grant matrix denial-path coverage. | `SUBSYSTEM_STORAGE.md`, `SUBSYSTEM_HISTORY.md`, `SUBSYSTEM_SECURITY.md`, persistence/history/security runtime code | Subsystem health summaries and critical integrity/denial-path tests are in CI or documented as explicit follow-ons. |
| 9 | **`lane:test-infra` (`#97`)** | Test scaling friction is now slowing safe refactors and subsystem closure. | Land `ACTIVE_CAPABILITIES` test-safe path, `test-utils` feature, `[[test]] scenarios` binary, incremental scenario migration, CI job split. | `2026-02-26_test_infrastructure_improvement_plan.md`, `registries/infrastructure/mod_loader.rs`, `Cargo.toml`, `tests/scenarios/` (new) | New scenarios test binary runs in CI and high-value scenario cases start moving out of ad hoc placements. |
| 10 | **`lane:knowledge-capture` (`#98`)** | UDC/semantic organization, badges/tags, import, and clipping are strategically aligned but mostly still design-level or partial. | UDC semantic physics/workbench grouping, layout injection hook + Magnetic Zones prerequisites, badges/tags MVP, import and clipping MVPs. | `2026-02-23_udc_semantic_tagging_plan.md`, `2026-02-24_layout_behaviors_plan.md`, `2026-02-20_node_badge_and_tagging_plan.md`, `2026-02-11_*_plan.md` | One end-to-end knowledge capture path (import/clip -> tag/UDC -> visible graph/workbench effect) is shipped. |

### Core vs Incubation Note

- `lane:verse-intelligence` is intentionally tracked in `1A` as an incubation lane (parallel / non-blocking for Graphshell core completion).
- It should still get a hub issue + child issues soon, but not ahead of stabilization, control UI/settings, and embedder debt retirement.

---

## 1D. Prospective Lane Catalog (Comprehensive)

This is the complete lane catalog for near/mid-term planning. `§1C` is the prioritized execution board; this section is the fuller universe so good ideas do not disappear between audits.

### A. Active / Immediate Lanes (Execution Now)

| Lane | Scope | Status | Primary Docs / Hotspots | Notes |
| --- | --- | --- | --- | --- |
| `lane:stabilization` (`#88`) | User-visible regressions, control responsiveness, focus affordances, camera/lasso correctness | Active when regressions exist | `render/mod.rs`, `app.rs`, `gui.rs`, `input/mod.rs`, `tile_compositor.rs` | Preempts other lanes while an active repro exists. |
| `lane:roadmap` | Docs/planning issues `#11/#12/#13/#14/#18/#19` | Active merge-safe default | `IMPLEMENTATION_ROADMAP.md`, planning docs | Low conflict background lane. |
| `lane:control-ui-settings` (`#89`) | Command surfaces + settings IA/surface execution | Active planning / queued (high priority) | `2026-02-24_control_ui_ux_plan.md`, `2026-02-20_settings_architecture_plan.md`, `render/command_palette.rs` | User report now provides concrete issue-ready slices (palette/context unification, theme toggle, omnibar/radial polish). |
| `lane:embedder-debt` (`#90`) | Servoshell inheritance retirement / host-UI decomposition | Prospective (high priority, active child slices) | `2026-02-20_embedder_decomposition_plan.md`, `gui.rs`, `gui_frame.rs`, `host/*` | Includes compositor callback pass contract and legacy webview context-menu/new-tab path retirement/bridging. |
| `lane:runtime-followon` (`#91`) | SR2/SR3 signal routing contract/fabric + observability | Prospective (ticket first) | `SYSTEM_REGISTER.md`, `TERMINOLOGY.md` | Requires fresh child issues; do not reuse queue-cleanup issues. |

### B. Core Platform / Architecture Completion Lanes

| Lane | Scope | Status | Primary Docs / Hotspots | Notes |
| --- | --- | --- | --- | --- |
| `lane:viewer-platform` (`#92`) | Universal content execution + real embedded viewers + Wry foundation | Prospective | `2026-02-24_universal_content_model_plan.md`, `2026-02-23_wry_integration_strategy.md`, `tile_behavior.rs`, `mods/native/verso/mod.rs`, `Cargo.toml` | Closes spec/code drift around viewer support and `viewer:wry`. |
| `lane:diagnostics` (`#94`) | AnalyzerRegistry, in-pane TestHarness, invariant/health surfacing | Prospective | `SUBSYSTEM_DIAGNOSTICS.md`, diagnostics runtime/pane code | Leverage multiplier for all other lanes. |
| `lane:subsystem-hardening` (`#96`) | Storage/history/security closure slices | Prospective | `SUBSYSTEM_STORAGE.md`, `SUBSYSTEM_HISTORY.md`, `SUBSYSTEM_SECURITY.md` | Can be split into sublanes once issue volume grows. |
| `lane:test-infra` (`#97`) | T1/T2 scaling, `test-utils`, scenario binary, CI split | Prospective | `2026-02-26_test_infrastructure_improvement_plan.md`, `mod_loader.rs`, `Cargo.toml` | Prefer infra-only PRs to reduce merge risk. |
| `lane:accessibility` (`#95`) | WebView bridge closure + Graph Reader + inspector + focus/nav contracts | Prospective | `SUBSYSTEM_ACCESSIBILITY.md`, `tile_behavior.rs`, `gui.rs` | Includes placeholder inspector replacement. |

### C. UX / Interaction / Graph Capability Lanes

| Lane | Scope | Status | Primary Docs / Hotspots | Notes |
| --- | --- | --- | --- | --- |
| `lane:knowledge-capture` (`#98`) | UDC organization, import, clipping, badges/tags, visible graph effects | Prospective | `2026-02-23_udc_semantic_tagging_plan.md`, `2026-02-24_layout_behaviors_plan.md`, `2026-02-20_node_badge_and_tagging_plan.md`, `2026-02-11_*_plan.md` | Canonical “capture + classify + surface” lane. |
| `lane:layout-semantics` | Layout injection hook, Magnetic Zones prerequisites and execution; workbench/workspace/tile semantic distinctions | Prospective (design pressure increasing) | `2026-02-24_layout_behaviors_plan.md`, `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` | User report surfaced unresolved tile vs workspace semantics and need for a root workbench overview UX. |
| `lane:performance-physics` | Culling, LOD, physics responsiveness/reheat, policy tuning | Partial / follow-on | `2026-02-24_performance_tuning_plan.md`, `2026-02-24_physics_engine_extensibility_plan.md` | Some slices landed; keep as follow-on lane for deeper performance + policy work. |
| `lane:command-surface-parity` | Omnibar/palette/radial/menu trigger parity and command discoverability | Prospective | `GRAPHSHELL_AS_BROWSER.md`, control UI UX docs, `render/command_palette.rs` | Can remain under `control-ui-settings` unless scope expands. |
| `lane:graph-ux-polish` | Multi-select, semantic tab titles, small high-leverage graph interactions | Prospective / quick-slice feeder | `2026-02-18_graph_ux_research_report.md`, `2026-02-23_graph_interaction_consistency_plan.md` | Good feeder lane for low-risk UX improvements between bigger slices. |

### D. Staged Feature / Roadmap Adoption Lanes (Post-Core Prereqs)

These are mostly sourced from the forgotten-concepts table and adopted strategy docs. They should be explicitly tracked as lanes once prerequisites are met.

| Lane | Scope | Trigger / Prereq | Primary Docs | Notes |
| --- | --- | --- | --- | --- |
| `lane:history-stage-f` | Temporal Navigation / Time-Travel Preview (Stage F) | Stage E history maturity + preview isolation hardening | `2026-02-20_edge_traversal_impl_plan.md`, `SUBSYSTEM_HISTORY.md` | Treat as staged backlog lane, not a quick feature. |
| `lane:presence-collaboration` | Collaborative presence (ghost cursors, follow mode, remote selection) | Verse sync + identity/presence semantics stable | `design_docs/verse_docs/implementation_strategy/2026-02-25_verse_presence_plan.md` | Crosses Graphshell + Verse; likely needs dedicated hub. |
| `lane:lens-physics` | Progressive lenses + lens/physics binding policy execution | Runtime lens resolution + distinct physics preset behavior | `2026-02-25_progressive_lens_and_physics_binding_plan.md`, interaction/physics docs | Can begin with policy wiring before full UX polish. |
| `lane:doi-fisheye` | Semantic fisheye / DOI implementation | Basic LOD + viewport culling stable | `2026-02-25_doi_fisheye_plan.md`, graph UX research | Visual ergonomics lane; pair with diagnostics/perf instrumentation. |
| `lane:visual-tombstones` | Ghost nodes/edges after deletion | Deletion/traversal/history UX stable | `2026-02-25_visual_tombstones_plan.md` | Adopted concept with strategy doc; candidate early roadmap lane. |
| `lane:omnibar` | Unified omnibar (URL + graph search + web search) | Command palette/input routing stabilized | `GRAPHSHELL_AS_BROWSER.md`, graph UX research | Core browser differentiator; keep distinct from palette cleanup. |
| `lane:view-dimension` | 2D↔3D hotswitch + position parity | Pane/view model + graph view state stable | `2026-02-24_physics_engine_extensibility_plan.md`, `PROJECT_DESCRIPTION.md` | Future-facing but should remain visible in planning. |
| `lane:html-export` | Interactive HTML export | Viewer/content model + snapshot/export shape defined | archived philosophy + browser docs | Strong shareability lane; non-core until model/export safety is defined. |

### E. Verse / Intelligence Incubation Lanes (Design-to-Code)

| Lane | Scope | Status | Primary Docs | Notes |
| --- | --- | --- | --- | --- |
| `lane:verse-intelligence` (`#93`) | Hub lane for model slots + adapters + conformance + portability + archetypes | Design-ready / issue hub open | `2026-02-26_model_slots_adapters_udc_personalization_plan.md` | Start with schemas/contracts + slot binding + diagnostics, not training. |
| `lane:intelligence-memory` | STM/LTM + engram memories + extractor/ingestor + ectoplasm interfaces | Design-ready / issue hub missing | `2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md` | May be tracked as a child lane under `lane:verse-intelligence`. |
| `lane:model-index-verse` | Requirements/benchmarks/community reports evidence registry for model selection/diets | Conceptual / partially documented | model slots plan (Model Index sections), local intelligence research | Evidence substrate for archetypes and conformance decisions. |
| `lane:adapter-portability` | LoRA extraction/import/export, portability classes, reverse-LoRA tooling integration | Design-ready / issue hub missing | model slots plan (`TransferProfile`, portability classes) | Likely late-phase child lane after schemas + evals exist. |
| `lane:archetypes` | Archetype presets, nudging, “Design Your Archetype”, derivation from existing models | Design-ready / issue hub missing | model slots plan (`ArchetypeProfile`) | Keep modular and non-blocking to core Graphshell. |

### F. Maintenance / Quality Governance Lanes (Keep Explicit)

| Lane | Scope | Status | Notes |
| --- | --- | --- | --- |
| `lane:spec-code-parity` (`#99`) | Reconcile docs/spec claims vs code reality (viewers, Wry, placeholders, status flags) | Ongoing | Use this when mismatches pile up; often docs-only, sometimes tiny code fixes. |
| `lane:queue-hygiene` | Issue state reconciliation, closure receipts, register refreshes | Ad hoc (recently exercised) | Keep rare and bounded; should support execution, not replace it. |
| `lane:docs-canon` | Terminology/architecture canon cleanup across `TERMINOLOGY.md`, `SYSTEM_REGISTER.md`, subsystem guides | Ad hoc | Use when implementation changes invalidate routing/authority language. |

### Catalog Usage Rules

- Add new lanes here before or at the same time they appear in `§1A` sequencing.
- Promote a lane into `§1C` only when it has a clear execution window, owner hotspot set, and issue stack (or an explicit issue-creation slice).
- Do not remove future-facing lanes just because they are blocked; mark the blocker and trigger instead.

---

## 2. Top 10 Forgotten Concepts for Adoption (Vision / Research Ideas Missing from Active Queue)

These are not "do now" items. They are concepts that should be explicitly adopted into planning so they do not disappear between migration and feature work.

| Rank | Forgotten Concept | Adoption Value | Source Docs | Adoption Trigger |
| --- | --- | --- | --- | --- |
| 1 | **Visual Tombstones (ghost nodes/edges after deletion)** | Preserves structural memory and reduces disorientation after destructive edits. | `2026-02-24_visual_tombstones_research.md` | After traversal/history UI and deletion UX are stable. |
| 2 | **Temporal Navigation / Time-Travel Preview** | Makes traversal history and deterministic intent log materially useful to users (not just diagnostics). | `2026-02-20_edge_traversal_impl_plan.md` (Stage F), `GRAPHSHELL_AS_BROWSER.md`, `2026-02-18_graph_ux_research_report.md` | After Stage E History Manager closure and preview-mode effect isolation hardening. |
| 3 | **Collaborative Presence (ghost cursors, remote selection, follow mode)** | Turns Verse sync from data sync into shared work. | `2026-02-18_graph_ux_research_report.md` §15.2, `GRAPHSHELL_AS_BROWSER.md`, Verse vision docs cited there | After Phase 5 done gates and identity/presence semantics are stable. |
| 4 | **Semantic Fisheye + DOI (focus+context without geometric distortion)** | High-value readability improvement for dense graphs; preserves mental map while surfacing relevance. | `2026-02-18_graph_ux_research_report.md` §§13.2, 14.8, 14.9 | After basic LOD and viewport culling are in place. |
| 5 | **Magnetic Zones / Group-in-a-Box / Query-to-Zone** | Adds spatial organization as a first-class workflow, not just emergent physics. | `2026-02-24_layout_behaviors_plan.md` Phase 3 (expanded with persistence scope, interaction model, and implementation sequence), `2026-02-18_graph_ux_research_report.md` §13.1 | **Prerequisites now documented** in `layout_behaviors_plan.md` §3.0–3.5. Implementation blocked on: (1) layout injection hook (Phase 2), (2) Canonical/Divergent scope settlement. Trigger: when both blockers are resolved, execute implementation sequence in §3.5. |
| 6 | **Graph Reader ("Room" + "Map" linearization) and list-view fallback** | Critical accessibility concept beyond the initial webview bridge; gives non-visual users graph comprehension. | `2026-02-24_spatial_accessibility_research.md`, `SUBSYSTEM_ACCESSIBILITY.md` §8 Phase 2 | After Phase 1 WebView Bridge lands. |
| 7 | **Unified Omnibar (URL + graph search + web search heuristics)** | Core browser differentiator; unifies navigation and retrieval. | `GRAPHSHELL_AS_BROWSER.md` §7, `2026-02-18_graph_ux_research_report.md` §15.4 | After command palette/input routing stabilization. |
| 8 | **Progressive Lenses + Lens/Physics binding policy** | Makes Lens abstraction feel native and semantic, not static presets. | `2026-02-24_interaction_and_semantic_design_schemes.md`, `2026-02-24_physics_engine_extensibility_plan.md` (lens-physics binding preference) | After Lens resolution is active runtime path and physics presets are distinct in behavior. |
| 9 | **2D↔3D Hotswitch with `ViewDimension` and position parity** | Named first-class vision feature; fits the new per-view architecture and future Rapier/3D work. | `2026-02-24_physics_engine_extensibility_plan.md`, `design_docs/PROJECT_DESCRIPTION.md` | After pane-hosted view model and `GraphViewState` are stable. |
| 10 | **Interactive HTML Export (self-contained graph artifact)** | Strong shareability and offline review workflow; distinctive output mode. | `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md` (archived concept) | After viewer/content model and export-safe snapshot shape are defined. |

Appended adoption note (preserved from PR `#55`, pending table refactor):
- Visual Tombstones (`Rank 1`) is now backed by `design_docs/graphshell_docs/implementation_strategy/2026-02-25_visual_tombstones_plan.md` and should be treated as `✅ adopted` in future table cleanup.

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

---

## 3. Top 10 Quickest Improvements (Low-Effort / High-Leverage Slices)

These are intentionally scoped to small slices that can ship independently without waiting for larger architecture work.

| Rank | Quick Improvement | Why It Pays Off | Primary Source Docs |
| --- | --- | --- | --- |
| 1 | **Extract `desktop/radial_menu.rs` from `render/mod.rs`** | Reduces render module sprawl and unblocks control UI redesign without behavior changes. | `2026-02-24_control_ui_ux_plan.md` |
| 2 | **Extract `desktop/command_palette.rs` from `render/mod.rs`** | Same benefit as #1; clarifies ownership for unified command surface work. | `2026-02-24_control_ui_ux_plan.md` |
| 3 | **Reheat physics on `AddNode` / `AddEdge`** | Fixes "dead graph" feel immediately when physics is paused. | `2026-02-24_layout_behaviors_plan.md` §1.1, `2026-02-18_graph_ux_research_report.md` §5.3 |
| 4 | **Spawn new nodes near semantic parent (parent + jitter)** | Improves mental-map preservation and reduces convergence churn. | `2026-02-24_layout_behaviors_plan.md` §1.2, `2026-02-18_graph_ux_research_report.md` §§2.1, 2.6 |
| 5 | **Fix `WebViewUrlChanged` prior-URL ordering in traversal append path** | Prevents incorrect traversal records and future temporal-navigation corruption. | `2026-02-20_edge_traversal_impl_plan.md`, `2026-02-20_edge_traversal_model_research.md` |
| 6 | **Wire `Ctrl+Click` multi-select in graph pane** | Tiny code slice with immediate UX gain; unlocks group operations expectations. | `2026-02-18_graph_ux_research_report.md` §§1.3, 6.3 |
| 7 | **Add semantic container tab titles (`Split ↔`, `Split ↕`, `Tab Group`, `Grid`)** | Converts "looks broken" tile labels into teachable architecture UI. | `2026-02-23_graph_interaction_consistency_plan.md` Phase 4 |
| 8 | **Add zoom-adaptive label LOD thresholds (hide/domain/full)** | Immediate clarity and performance win at low zoom, low implementation risk. | `2026-02-24_performance_tuning_plan.md` Phase 2.1, `2026-02-18_graph_ux_research_report.md` §7.3 |
| 9 | **Add `ChannelSeverity` to diagnostics channel descriptors** | Small schema extension that unlocks better pane prioritization and health summary. | `2026-02-24_diagnostics_research.md` §4.6, §7 |
| 10 | **Add/confirm `CanvasRegistry` culling + LOD policy toggles** | Minimal schema/policy work that unblocks performance slices and keeps behavior policy-driven. | `2026-02-24_performance_tuning_plan.md`, `2026-02-22_registry_layer_plan.md` |

### Quick Win Notes

- Items 1-2 pair naturally and should be landed together if the extraction is mechanical.
- Items 3-5 are correctness/feel fixes and should not wait for full layout/traversal phases.
- Items 9-10 are low-churn infrastructure improvements that improve future implementation discipline.

---

## 4. Recommended Execution Sequence (2026-02-25 Refresh)

Historical reference only (retained for archive continuity). Superseded by:
- `§1A` Merge-Safe Lane Execution Reference (current canonical sequencing)
- `§1C` Top 10 Active Execution Lanes (current strategic lane board)

### Wave A: Close Migration Done Gates (Highest Risk Reduction)

1. Registry Phase 5.4 closure (delta sync harness + conflict diagnostics)
2. Registry Phase 5.5 closure (access control harness + deny-path coverage)
3. Registry Phase 6.4 canonical imports/path cleanup
4. Registry Phase 6.5 shim removal + final boundary lock + doc path refresh

### Wave B: Establish Pane/View and Viewer Foundations

1. Pane-hosted multi-view architecture doc+type sync (graph/viewer/tool pane model)
2. Graph multi-view implementation (`GraphViewId`, per-view state, split/lens UI)
3. Universal content Steps 1-3 (data model + viewer selection + plaintext baseline)
4. Wry Steps 1-5 (feature gate through lifecycle integration)

### Wave C: UX Consolidation, Scale, and Accessibility Baselines

1. Control UI/UX extraction + ActionRegistry routing
2. Viewport culling + LOD policy activation
3. WebView accessibility bridge (Phase 1 critical fix)
4. Pull from Quick Wins list continuously between larger slices

---

## 5. Registry Plan Closure Backlog (Audited 2026-02-24, retained 2026-02-25)

This is the strict closure checklist derived from the current `2026-02-22_registry_layer_plan.md` state and code/test audit.

### 5.1 Phase 5.4 — Delta Sync Done-Gate Closure

1. **Add missing harness scenario `verse_delta_sync_basic`**
   - Create scenario under `desktop/tests/scenarios/` and include it in `desktop/tests/scenarios/mod.rs`.
   - Validate two-instance flow: node created on A appears on B within 5 seconds.
   - Validate concurrent rename conflict resolves deterministically (LWW behavior) without crash.

2. **Close conflict diagnostics gap in runtime code**
   - Implement emission paths for `verse.sync.conflict_detected` and `verse.sync.conflict_resolved` where conflict logic runs.
   - Ensure channels are seeded/registered in diagnostics registry defaults and covered by contract tests.

3. **Acceptance checks (must all pass)**
   - `cargo test verse_delta_sync_basic`
   - Diagnostics assertions include `unit_sent`, `unit_received`, `intent_applied`, `conflict_detected`, `conflict_resolved`.
   - `cargo check` remains green.

### 5.2 Phase 5.5 — Workspace Access Control Done-Gate Closure

1. **Add missing harness scenario `verse_access_control`**
   - Validate grant matrix for `ReadOnly` and `ReadWrite` workspace permissions.
   - Confirm read-only peer receives remote updates but local mutating intents for that workspace are rejected.

2. **Harden access-denied behavior and coverage**
   - Ensure inbound non-granted workspace sync always emits `verse.sync.access_denied` and does not mutate graph state.
   - Add focused tests for deny paths and revoke/forget flows.

3. **Acceptance checks (must all pass)**
   - `cargo test verse_access_control`
   - Access-control path emits `verse.sync.access_denied` deterministically.
   - `cargo check` remains green.

### 5.3 Phase 6.4 — Filesystem/Import Canonicalization Closure

1. **Finish canonical imports away from root compatibility paths**
   - Remove remaining `crate::persistence::*` consumers by migrating to `crate::services::persistence::*` (and `types` submodule path equivalents) in runtime/UI/tests where appropriate.
   - Continue mechanical path migration slices per subtree with compile validation after each slice.

2. **Align test/harness imports during each move slice**
   - Update `desktop/tests/scenarios/*` imports in the same commit as each path migration.
   - Keep boundary/seam contracts green after each move.

3. **Acceptance checks (must all pass)**
   - `cargo check` after each subtree slice.
   - `cargo test contract_runtime_layers_do_not_call_graph_topology_mutators_directly`
   - `cargo test servo_callbacks_only_enqueue_events`

### 5.4 Phase 6.5 — Transition Shim Removal & Final Boundary Lock

1. **Delete all temporary root re-export shims**
   - Remove shim files/usages for: `running_app_state.rs`, `window.rs`, `search.rs`, `persistence/mod.rs` (root compatibility layer).
   - Update all callsites to canonical module paths before deleting shims.

2. **Enforce single-write-path visibility target**
   - Tighten graph topology mutator visibility in `model/graph/mod.rs` to the planned boundary level and resolve resulting callers through reducer-owned paths.

3. **Update docs to canonical paths**
   - Refresh strategy/architecture map references that still point at shim or pre-move paths.

4. **Acceptance checks (must all pass)**
   - No transition shims remain at crate root.
   - Full suite passes: `cargo test` and `cargo check`.
   - Registry done-gate language in strategy docs matches repository reality.

### 5.5 Immediate Next Sequence (Recommended Order)

1. Implement `verse_delta_sync_basic` + conflict diagnostics channels.
2. Implement `verse_access_control` harness and deny-path assertions.
3. Complete remaining 6.4 import canonicalization (`persistence` path cleanup).
4. Execute 6.5 shim removal in one controlled slice with full-suite validation.

---

---

## Reference Payload (Preserved Numbering / Historical Layout)

The sections below retain their original numbering and structure for continuity with prior receipts/PRs. Treat them as reference material unless explicitly promoted into `§1A` / `§1C` / `§1D`.

## 2. Backlog Ticket Stubs

_Source file before consolidation: `2026-02-25_backlog_ticket_stubs.md`_


**Status**: Active index (detailed payload moved to archive receipts)
**Purpose**: Keep this register readable as the active control-plane while preserving detailed ticket stubs elsewhere.

### Canonical sources

- Primary historical stubs snapshot:
  - `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_backlog_ticket_stubs.md`
- Sequencing receipt (conflict-aware lane/stack plan):
  - `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_planning_register_lane_sequence_receipt.md`

### Usage rule

- Use Sections `1`, `1A`, and `1B` in this file for current execution decisions.
- Use archived ticket stubs only for deep scope/details while drafting new issues.
- When execution sequencing changes materially, append a new dated receipt (do not re-expand this active file).

---

## 3. Implementation Guides

_Source file before consolidation: `2026-02-25_copilot_implementation_guides.md`_

**Status**: Active index (detailed implementation notes moved to archive)
**Purpose**: Keep agent-facing guidance discoverable without keeping long branch-specific instructions inline.

### Canonical sources

- Archived copilot implementation guide snapshot:
  - `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_copilot_implementation_guides.md`
- Historical backlog source:
  - `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_backlog_ticket_stubs.md`

### Additional active plans (linked for coverage; not all are prioritized in Section 1)

- `2026-02-11_clipping_dom_extraction_plan.md` (Verso/DOM extraction feature slice)
- `2026-02-20_node_badge_and_tagging_plan.md` (badge/tag visual + interaction layer)
- `2026-02-20_settings_architecture_plan.md` (settings pane/page model and orchestration direction)
- `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` (workbench pane/tab semantics and promotion model)
- `2026-02-23_udc_semantic_tagging_plan.md` (UDC semantic clustering/tagging roadmap)
- `2026-02-25_interactive_html_export_plan.md` (deferred export artifact plan; adopted concept)

### Usage rule

- For active coding tasks, treat issue threads + current strategy docs as source of truth.
- Use archived copilot guides as historical implementation hints only; validate against current code before applying.

---

## 4. Suggested Tracker Labels (Operational Defaults)

- Priority tasks: `priority/top10`, `architecture`, `registry`, `viewer`, `ui`, `performance`, `a11y`
- Roadmap adoption: `concept/adoption`, `research-followup`, `future-roadmap`
- Quick wins: `quick-win`, `low-risk`, `refactor`, `ux-polish`, `diag`

## 5. Import Notes (Short Form)

- Keep `P#`, `F#`, `Q#` prefixes aligned between docs and tracker.
- Prefer one issue per mergeable slice in hotspot files.
- If a ticket body exceeds practical review size, move extended detail into a timestamped archive receipt and link it.
