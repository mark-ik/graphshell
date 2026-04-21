<!--
SPDX-License-Identifier: MPL-2.0
-->

# Verso Shell Authority Refactor Plan

**Date**: 2026-04-21  
**Status**: Proposed refactor plan  
**Owner**: Shell / Workbench / Viewer Platform  
**Scope**: Promote `verso` from “emerging cross-engine helper crate + legacy native mod name” into the Shell’s explicit routing authority for engine choice, viewer choice, pane ownership, and backend escalation. Decompose the current `mods/native/verso` bundle so backend/provider code no longer masquerades as the authority.

## 1. Why This Refactor Exists

The current tree has two different things called “Verso”:

- `crates/verso` is now the emerging authority layer for cross-engine decisions.
- `mods/native/verso` is a provider bundle containing Servo/Wry integration, Gemini/Gopher/Finger servers, and browser-adjacent runtime helpers.

That split is confusing because the name `verso` semantically fits the first role much better than the second:

- `verso://` is already the app’s internal shell/workbench namespace.
- viewer/backend choice is now increasingly shell policy rather than mod-local behavior.
- `middlenet-engine` has already started shedding cross-engine concerns into `crates/verso`.

The architectural direction should therefore be:

- **`verso` becomes a Shell subsystem**
- **backend/provider code stops owning the `verso` name**

## 1.1 Current state (verified 2026-04-21)

Premises for this refactor were audited against the codebase on 2026-04-21 (see conversation log). Summary of the actual state — not all of it matches the plan's initial framing, so sequencing in §5/§7 below has been revised accordingly.

- `crates/verso/src/lib.rs` exports the right shape (`EngineChoice`, `EngineOverride`, `WebEnginePreference`, `HostCapabilities`, `VersoRequest`, `DispatchOutcome`, `ViewerRoutingDecision`, `dispatch_request`, `dispatch_prepared`, `select_viewer_for_content`). Phase 0's "confirm public types" is ~80% done already.
- `crates/middlenet-engine/src/engine.rs` is already Middlenet-internal-only — no Servo/Wry/viewer-id awareness. DoD #2 is *already true*.
- `mods/native/verso/mod.rs` is cleanly a provider bundle (Wry lifecycle + Gemini/Gopher/Finger + storage); no routing authority code lives here. The rename is safe.
- `crates/verso` → `middlenet-engine` is one-way; no circular dependency risk.
- "mod:verso" appears as a string literal in only 8 sites (infrastructure + tests). The manifest-id rename is cheap and can be done at any time.

The load-bearing caveat: `verso` is **not yet the primary authority**. At [`shell/desktop/workbench/tile_runtime.rs:81–108`](../../../../shell/desktop/workbench/tile_runtime.rs#L81-L108), `preferred_viewer_id_for_content` currently:

1. calls `phase0_select_viewer_for_content()` (registry baseline) **first**,
2. then calls `::verso::select_viewer_for_content()`,
3. then falls back to the registry result if verso returned `None`.

That shape is "verso as fallback consultant, registry as default." Risk 8.3 (registry/runtime divergence) is not a future risk — it is the current state. Viewer-id string literals appear 202× across 27 files; `shell/desktop/runtime/registries/mod.rs` alone carries 35 of them. Closing the inversion is the highest-leverage move in this refactor — and the reason Phases 3+5 have been merged and moved to the front of the ordered list below.

## 2. Target Architecture

### 2.1 `verso` owns authority

`crates/verso` should become the canonical authority for:

- content routing across Middlenet / Servo / Wry
- viewer selection for pane-backed content
- browser-backend preference interpretation
- per-pane engine ownership
- backend escalation and fallback policy
- shell-facing routing outcomes and diagnostics reasons

`verso` should **not** own:

- rendering implementation
- protocol parsing/adaptation
- transport stacks
- graph domain logic
- platform backend details

It is an orchestration layer, not a renderer or runtime backend.

### 2.2 Middlenet stays Middlenet

`middlenet-engine` should continue narrowing toward:

- semantic adaptation
- Middlenet-internal lane choice
- Direct / Html / FaithfulSource / Unsupported
- prepared-document packaging for Middlenet-owned surfaces

It should not know how to escalate into Servo/Wry.

### 2.3 Provider bundle gets renamed

The current `mods/native/verso` bundle should stop being the architectural “Verso”.

It should be renamed toward something that reflects what it actually provides:

- `mods/native/web_runtime`
- `mods/native/browser_backends`
- `mods/native/web_backends`
- `mods/native/verso_native`

Recommended default: **`mods/native/web_runtime`**.

That bundle would provide:

- Servo webview/runtime integration
- Wry overlay/runtime integration
- local Gemini/Gopher/Finger server helpers
- browser-adjacent storage/runtime helpers

Those capabilities are inputs into `verso`, not the authority itself.

## 3. Current Hot Spots

The current code already points at the right seam:

- `crates/verso/src/lib.rs`
  - cross-engine dispatch and viewer routing are starting to live here.
- `crates/middlenet-engine/src/engine.rs`
  - Middlenet lanes are now internal-only.
- `shell/desktop/workbench/tile_runtime.rs`
  - pane-level effective viewer choice now calls into `verso`.

The biggest remaining places where “Verso the authority” and “Verso the mod” are still conflated:

- `mods/native/verso/mod.rs`
- `registries/infrastructure/mod_activation.rs`
- `registries/infrastructure/mod_loader.rs`
- `shell/desktop/runtime/registries/mod.rs`
- `shell/desktop/workbench/tile_behavior.rs`
- `shell/desktop/workbench/tile_compositor.rs`
- `shell/desktop/ui/workbench_host.rs`
- app settings/persistence that still encode backend choice directly as `viewer:webview` / `viewer:wry`

## 4. Refactor Goals

This refactor is complete when the following are true:

1. `verso` is the named shell authority for viewer/backend/engine routing.
2. `mods/native/verso` no longer exists under that name.
3. backend/provider code is renamed to a capability/provider-oriented name.
4. pane routing and effective viewer choice go through `verso`.
5. shell/runtime registry helpers do not hardcode web-backend policy independently of `verso`.
6. Middlenet remains ignorant of Servo/Wry.
7. docs and terminology stop describing “Verso” as merely a mod.

## 5. Ordered Refactor Phases

### Phase 0 (pre-work): Largely already done

- Public type shape in `crates/verso` matches the target contract (see §1.1). The few items still missing — an `VersoResolvedRoute`/`VersoPaneOwner`/`VersoRouteReason` trio — will be added under Phase 3 below, *after* consolidation pressure surfaces their real shape, not speculatively.
- `crates/middlenet-engine` is already Middlenet-internal-only. DoD #2 is satisfied without work.

### Phase 1: Make `verso` the primary authority at call sites

This is the highest-leverage phase and is what actually turns `verso` from "fallback consultant" into "routing authority." Prior versions of this plan had this as Phase 3+5; it is moved to the front because the rename and the resolved-route types without this step would only reorganize a still-inverted flow.

Concrete edits:

- [`shell/desktop/workbench/tile_runtime.rs:81–108`](../../../../shell/desktop/workbench/tile_runtime.rs#L81-L108) — inside `preferred_viewer_id_for_content`, call `::verso::select_viewer_for_content()` **first**. Only consult `phase0_select_viewer_for_content()` when verso returns `None` (content verso doesn't route: specialized non-web viewers — images, PDFs, etc.).
- `shell/desktop/runtime/registries/mod.rs` — trim the registry's web-backend policy to capability description. Non-web viewer selection (images, PDFs, local files) stays owned by the registry; web and Middlenet content now flow entirely through verso.
- Existing scattered `viewer:webview` / `viewer:wry` / `viewer:middlenet` string comparisons at call sites that were previously reading registry output become redundant for web content — they can be removed as the call sites migrate.

Acceptance: every browser-backed and Middlenet-backed routing decision has a verso invocation as its primary source, and the fallback path from verso into the registry never returns a web viewer id.

### Phase 2: Rename the native provider bundle

Rename:

- `mods/native/verso/` → `mods/native/web_runtime/`

Then update:

- `mods/native/mod.rs`
- all `crate::mods::native::verso::*` imports
- mod activation tables
- manifest wiring
- docs/tests referencing the old path

Important distinction:

- **mod id can remain `mod:verso` temporarily** if migration cost needs to stay low (the manifest-id rename is queued as a later optional step since only 8 literal references exist).
- file/module namespace should still be renamed first.

This phase is cheap (~10 files, 8 literal references) and independent of Phase 1 — it can land at any time after Phase 1 is stable, or before if an orchestrated rename window opens up.

### Phase 3: Introduce resolved-route / pane-owner state

Now that Phase 1 has consolidated the decision point, the shape of a "resolved route" is obvious from actual usage — not speculative. Introduce:

- `VersoResolvedRoute` — decision + reason + override provenance
- `VersoPaneOwner` — per-pane engine ownership handle
- `VersoRouteReason` — human-readable explanation for UX/debug surfaces

Thread these through pane/route state in the workbench. This reduces the remaining scattered raw-string checks (`viewer:webview` / `viewer:wry` / `viewer:middlenet`) to opaque handles behind the resolved-route type.

Introducing these types *before* Phase 1 would risk a god-object shape (Risk 8.1 inverted) because nothing constrains what they carry. Doing it *after* Phase 1 lets the consolidation dictate the fields.

### Phase 4: Settings + UX migration

Map user-facing concepts onto `verso`'s decision types:

- "Compat mode" means "prefer Wry through `verso`"
- Default web backend setting maps to `WebEnginePreference`
- Overview/debug surfaces show: engine owner, viewer backend, route reason, override state

This phase overlaps in scope with the settings-persistence migration (see `app/settings_persistence.rs` track). Coordinate to land changes together; do not parallel-land without alignment.

Longer-term: persist semantic preference (`WebEnginePreference`) rather than raw viewer ids.

### Phase 5: Terminology and docs

Update `design_docs/TERMINOLOGY.md` so "Verso" is described as:

- shell routing authority
- internal namespace owner
- viewer/pane/engine orchestration layer

And describe the renamed native mod as a backend/provider bundle, not the authority.

Priority docs to update:

- `design_docs/TERMINOLOGY.md`
- shell/workbench specs
- viewer backend docs
- Wry integration spec
- Middlenet lane docs
- mod architecture docs

### Phase 6 (optional): Rename the manifest id

Decide whether `mod:verso` should become `mod:web-runtime`. Only 8 literal references exist; it is a mechanical change that can ship after Phase 5 has normalized terminology. Gate on: whether any external consumer (settings, plugins, tests) has taken a hard dependency on the old id.

## 6. Concrete File Moves and Touch Points

### 6.1 Authority side

Primary authority crate:

- `crates/verso/src/lib.rs`

Likely shell wrappers or helpers:

- `shell/desktop/workbench/tile_runtime.rs`
- `shell/desktop/workbench/tile_behavior.rs`
- `shell/desktop/ui/workbench_host.rs`
- `shell/desktop/runtime/registries/mod.rs`

### 6.2 Provider/bundle side

Rename module tree:

- `mods/native/verso/mod.rs`
- `mods/native/verso/wry_manager.rs`
- `mods/native/verso/wry_types.rs`
- `mods/native/verso/wry_viewer.rs`
- `mods/native/verso/wry_frame_source.rs`
- `mods/native/verso/client_storage/*`
- `mods/native/verso/gemini/*`
- `mods/native/verso/gopher/*`
- `mods/native/verso/finger/*`

### 6.3 Infra and activation

- `registries/infrastructure/mod_activation.rs`
- `registries/infrastructure/mod_loader.rs`
- test helpers that disable `mod:verso`

### 6.4 Settings/persistence

- `app/settings_persistence.rs`
- `app/workspace_state.rs`
- any workbench UX that exposes browser-backend preference

Longer-term target:

- persist semantic preference (`WebEnginePreference`) rather than raw viewer ids where possible

## 7. Suggested PR Sequence

Revised 2026-04-21 after codebase verification. The principle: **land the authority inversion first**, because every other step becomes cleaner once `verso` owns the decision. Rename, type-introduction, and terminology updates are downstream consequences, not prerequisites.

### PR 1: Make `verso` primary at `tile_runtime.rs`

- Edit `preferred_viewer_id_for_content` to call `verso` first, consult registry only for non-web content.
- Touches 1-2 files in hot path; keeps existing tests as a regression gate.
- No new types, no rename churn.

### PR 2: Trim registry web-backend policy

- Walk `shell/desktop/runtime/registries/mod.rs` and strip web-backend selection from registry functions (leave capability description intact).
- Remove now-unreachable `viewer:webview` fallback branches at call sites downstream.

### PR 3: Rename `mods/native/verso` → `mods/native/web_runtime`

- Mechanical rename; keeps manifest id `mod:verso` for one migration window.
- Can land any time after PR 1 is stable; PR 2 is not a prerequisite.

### PR 4: Introduce resolved-route / pane-owner types

- Shape now obvious from PR 1/PR 2 usage.
- Thread through workbench pane state.
- Begin collapsing raw viewer-id string comparisons into resolved-route accessors.

### PR 5: Settings + UX migration

- Map "Compat mode" + default web backend settings to `WebEnginePreference`.
- Expose engine owner / route reason / override state in overview/debug surfaces.
- Coordinate with the in-flight settings-persistence track.

### PR 6: Terminology + docs cleanup

- Update `TERMINOLOGY.md`, shell/workbench specs, viewer backend docs, Wry integration spec, Middlenet lane docs, mod architecture docs.

### PR 7 (optional): Rename manifest id

- Decide `mod:verso` → `mod:web-runtime` only after PR 3/PR 6 have normalized terminology; gate on external-consumer dependency audit.

## 8. Risks and Guardrails

### 8.1 Risk: `verso` becomes a god object

Guardrail:

- keep it orchestration-only
- no rendering code
- no transport stacks
- no provider implementation logic

### 8.2 Risk: rename churn breaks too much at once

Guardrail:

- rename module path before changing manifest/capability identity
- separate naming migration from policy migration

### 8.3 Risk: registry/runtime and workbench diverge

Guardrail:

- every browser-backed content route should eventually be explainable through one `verso` decision object

### 8.4 Risk: viewer id strings remain the real authority

Guardrail:

- viewer ids remain backend handles, not policy truth
- policy truth should live in `verso` route/owner types

### 8.5 Risk: `ViewerRoutingDecision.viewer_id` still leaks string authority

Today `ViewerRoutingDecision` at [`crates/verso/src/lib.rs:92–96`](../../../../crates/verso/src/lib.rs#L92-L96) returns `viewer_id: &'static str` — a `"viewer:webview"` / `"viewer:wry"` / `"viewer:middlenet"` literal. Consumers are meant to treat these as opaque handles but every raw `==` string comparison downstream (tile_runtime.rs, registries/mod.rs, overview_plane.rs) reinforces string-as-authority habits.

Guardrail:

- Phase 4 should reshape `ViewerRoutingDecision` so consumers get a typed handle (e.g., `ViewerHandle` wrapping the string internally) plus `EngineChoice` and the reason, never a bare string.
- Call sites doing `decision.viewer_id == "viewer:wry"` are a red flag — they should be comparing `decision.engine` or asking the resolved route a typed question.

## 9. Definition of Done

This refactor is done when:

1. `verso` is documented and used as shell routing authority.
2. `middlenet-engine` contains no cross-engine escalation logic.
3. provider/backend code no longer lives under the `mods/native/verso` module path.
4. workbench pane routing goes through `verso` types, not scattered viewer-id heuristics.
5. settings and debug surfaces describe backend choice in `verso` terms.
6. terminology/docs no longer describe Verso primarily as a mod.

## 10. Immediate Recommendation

Revised 2026-04-21. Do the next slice in this order:

1. Land PR 1: make `verso` primary at `tile_runtime.rs:81–108`. This is the one edit that actually turns verso into authority. Everything else downstream gets cleaner.
2. Land PR 2: trim registry web-backend policy once PR 1 is stable.
3. Rename `mods/native/verso` → `mods/native/web_runtime` (PR 3). Cheap, independent of PR 1/PR 2 ordering after PR 1 has landed.
4. Introduce `VersoResolvedRoute` / `VersoPaneOwner` / `VersoRouteReason` (PR 4) with their real shape now visible.

Terminology cleanup (PR 6) follows naturally once the code has taken the new shape. The manifest-id rename (PR 7) is a later, opt-in step.

## 11. Progress log

### 2026-04-21 — PR 1 landed

- [`shell/desktop/workbench/tile_runtime.rs`](../../../../shell/desktop/workbench/tile_runtime.rs) `preferred_viewer_id_for_content` now calls `::verso::select_viewer_for_content()` first. The registry (`phase0_select_viewer_for_content`) is consulted only when verso returns `None` — i.e., for specialized non-web viewers (images, PDFs, local files) that verso legitimately doesn't route.
- The legacy two-step shape is gone:
  - No more pre-call to the registry.
  - No more "if selected.viewer_id != 'viewer:webview'" fallback branch (the registry no longer gets a chance to propose a web viewer in parallel with verso).
- `verso` is now the primary authority for web + Middlenet content at the workbench pane-routing decision point.

Verification:

- `cargo check -p graphshell` clean (only pre-existing warnings).
- `cargo test -p verso` — 7 pass.
- `cargo test -p graphshell --lib viewer` — 70 pass.
- Full `graphshell --lib` suite — 2166 pass / 0 fail / 3 ignored.

Not yet touched:

- PR 2: trim registry web-backend policy in [`shell/desktop/runtime/registries/mod.rs`](../../../../shell/desktop/runtime/registries/mod.rs). `phase0_select_viewer_for_content` still returns web viewer ids; consumers other than `tile_runtime.rs` still reach the registry-first path.
- `candidate_viewer_ids_for_node_pane` at [`tile_runtime.rs:510–546`](../../../../shell/desktop/workbench/tile_runtime.rs#L510-L546) still hardcodes `"viewer:webview"` / `"viewer:wry"` for the candidate list. Phase 4 (resolved-route types) will fold this into a typed enumeration.
- `tile_behavior.rs:545` reads the registry for accessibility inspector diagnostics — not a routing decision, left alone.

### 2026-04-21 — PRs 2, 3, 4 landed

#### PR 2 — Candidate enumeration now routes through verso

- [`shell/desktop/workbench/tile_runtime.rs`](../../../../shell/desktop/workbench/tile_runtime.rs) `candidate_viewer_ids_for_node_pane` no longer hardcodes `"viewer:webview"` / `"viewer:wry"` for http(s) URLs. It queries `::verso::select_viewer_for_content` twice (once per `WebEnginePreference`) to enumerate web + Middlenet candidates; the registry is consulted only for specialized non-web viewers.
- The registry's `select_for_uri` was left as-is. Routing callers no longer rely on its web-baseline output; the remaining web viewer returns are capability description consumed only by the accessibility inspector at `tile_behavior.rs:545` (diagnostic, not routing).
- Verification: `cargo test -p graphshell --lib viewer -- --test-threads=1` → 70/70.

#### PR 3 — Native provider bundle renamed

- `mods/native/verso/` → `mods/native/web_runtime/` on disk.
- [`mods/native/mod.rs`](../../../../mods/native/mod.rs) now declares `pub(crate) mod web_runtime;`.
- 29 bulk substitutions across 10 files for `mods::native::verso` → `mods::native::web_runtime`.
- 9 bare `verso::` call sites for the mod's wry helpers (`destroy_wry_overlay_for_node`, etc.) rewritten to `web_runtime::` (concentrated in [`tile_runtime.rs`](../../../../shell/desktop/workbench/tile_runtime.rs), [`tile_compositor.rs`](../../../../shell/desktop/workbench/tile_compositor.rs), [`lifecycle_reconcile.rs`](../../../../shell/desktop/lifecycle/lifecycle_reconcile.rs)), plus 4 test-only `verso::reset_wry_manager_for_tests` / `last_wry_overlay_sync_for_node_for_tests` sites.
- `"mod:verso"` string literals (8 total) kept as-is for compatibility — Phase 6 (manifest-id rename) remains optional/deferred.
- No changes to workspace-root `Cargo.toml` (the `crates/verso` library is a distinct thing from the renamed mod).

#### PR 4 — Resolved-route types in `crates/verso`

- Added to [`crates/verso/src/lib.rs`](../../../../crates/verso/src/lib.rs):
  - `VersoPaneOwner` — `Policy` / `UserPin` / `Unresolved`
  - `VersoRouteReason` — `MiddlenetLane(lane)` / `WebEnginePreferred(pref)` / `WebEngineFallback { preferred, used }` / `UserOverride` / `Unsupported`
  - `VersoResolvedRoute` — decision + reason + owner, with typed accessors (`is_wry()`, `is_middlenet()`, `engine()`, `viewer_id()`, `reason()`, `owner()`) so consumers can stop string-comparing viewer ids.
  - `resolve_route_for_content(uri, mime_hint, host_caps, preference, owner)` — preferred entry point; wraps `select_viewer_for_content` and populates reason/owner.
- 5 new unit tests cover: Middlenet lane reason, web-preference match, web-preference fallback, owner propagation, unsupported-content None.
- Types are introduced only; wiring them into `NodePaneState` / `preferred_viewer_id_for_content` is deferred to a follow-on PR (previously framed as "full migration"). Risk 8.5 (`ViewerRoutingDecision.viewer_id` as string authority) remains until that migration lands; the new typed accessors are the mechanism for the replacement.

#### Tests — combined

- `cargo test -p verso` → 12/12 (was 7 before PR 4; +5 new).
- `cargo test -p graphshell --lib viewer -- --test-threads=1` → 70/70.
- `cargo test -p graphshell --lib -- --test-threads=1` → 2164 pass / 2 known-flaky / 3 ignored. The two flakes (`phase0_registry_cancellation_short_circuits_before_viewer_selection` and `save_named_frame_bundle_preserves_collapsed_runtime_semantic_tabs`) pass when rerun in isolation; they depend on shared test state and are tracked under the archived 2026-04-19 flaky-test hygiene plan. Not regressions from PRs 2–4.

#### Not yet touched (carried forward)

- PR 5: settings + UX migration (map "Compat mode" / default web backend to `WebEnginePreference`; expose route reason in debug surfaces).
- PR 6: terminology + docs cleanup (`TERMINOLOGY.md`, shell/workbench specs, viewer backend docs).
- PR 7 (optional): manifest-id rename `mod:verso` → `mod:web-runtime`.
- Follow-on to PR 4: thread `VersoResolvedRoute` through `NodePaneState`; replace raw `viewer_id` string comparisons at the 17 identified call sites; update `ViewerRoutingDecision` to return a typed handle instead of `&'static str`.

### 2026-04-21 — PRs 5, 6, 7 landed (plan complete)

#### PR 5 — Settings + UX migration

- [`app/settings_persistence.rs`](../../../../app/settings_persistence.rs) — added `DefaultWebViewerBackend::web_engine_preference() -> ::verso::WebEnginePreference` method. The user-facing setting enum now exposes its verso mapping as a typed accessor; call sites stop open-coding the match.
- [`tile_runtime.rs preferred_viewer_id_for_content`](../../../../shell/desktop/workbench/tile_runtime.rs) — uses the new accessor. The registry-side preference→verso conversion is gone from the call site.
- [`workbench_host.rs WorkbenchNodeViewerSummary`](../../../../shell/desktop/ui/workbench_host.rs) — gained `verso_route: Option<::verso::VersoResolvedRoute>` field. `build_node_viewer_summary` populates it by calling `::verso::resolve_route_for_content` with owner derived from whether a user override exists (`UserPin` if overridden, otherwise `Policy`). Debug/overview surfaces can now read engine/reason/owner off a typed field instead of parsing viewer-id strings.
- "Compat mode" per §Phase 4 of the plan does not yet exist in the codebase (grep confirmed: no `CompatMode` / `compat_mode` references). Introducing it is deferred; the pattern established here (user setting → `WebEnginePreference`) is the template for when it lands.

#### PR 6 — Terminology + architecture docs

- [`design_docs/TERMINOLOGY.md`](../../../../design_docs/TERMINOLOGY.md) — "Verso" entry rewritten to frame verso as the shell's routing authority (`crates/verso` + the `verso://` internal namespace), with explicit pointer that the legacy "Verso mod" is now the `web_runtime` provider bundle feeding into it.
- [`design_docs/graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md`](../../../../design_docs/graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md) — Viewer feature table row "Servo/Web" updated to reference `web_runtime`, and a new "Viewer — Routing Authority" row added pointing at `crates/verso` as the decision-making layer. Viewer/Routing & Fallback row narrowed to describe non-web-only viewer registry responsibility.

#### PR 7 — Manifest-id rename

- `"mod:verso"` → `"mod:web-runtime"` at all 19 literal occurrences across 7 files (tests, mod_loader, mod_activation, runtime registry dispatcher, plus the web_runtime manifest itself).
- No persisted state reads the manifest id, so renaming is safe from a backward-compatibility standpoint. The dispatcher at [`shell/desktop/runtime/registries/mod.rs:1387`](../../../../shell/desktop/runtime/registries/mod.rs#L1387) now dispatches on `"mod:web-runtime" | "verso"` — the legacy `"verso"` short-form alias is kept intact.
- One test-site adjustment: the new `verso_route: None` field needed to be added to six `WorkbenchNodeViewerSummary` struct-literal sites in `overview_plane.rs` tests; awk-script injection handled it cleanly.

#### Tests — PR 5/6/7 combined

- `cargo test -p verso --lib` → 12/12.
- `cargo test -p graphshell --lib viewer -- --test-threads=1` → 70/70.
- `cargo test -p graphshell --lib workbench_host:: -- --test-threads=1` → 74/74.
- `cargo test -p graphshell --lib mod_loader:: -- --test-threads=1` → 19/19.
- `cargo test -p graphshell --lib persistence_ops -- --test-threads=1` → 26/26.
- `cargo test -p graphshell --lib -- --test-threads=1` → **2166 pass / 0 fail / 3 ignored**. (An interim run during the PR 7 test-wiring pass reported 24 ordering-dependent flakes; a clean rerun after the struct-literal fixes showed the suite green.)

#### Definition of Done — status

1. ✅ `verso` is documented and used as shell routing authority (PR 1 + PR 6 + this plan).
2. ✅ `middlenet-engine` contains no cross-engine escalation logic (was already true pre-plan; verified in §1.1).
3. ✅ Provider/backend code no longer lives under the `mods/native/verso` module path (PR 3).
4. ⚠️ Workbench pane routing goes through `verso` types (PR 1 + PR 2). ~17 raw `viewer_id` string comparisons remain at scattered call sites (`tile_compositor.rs`, `node_pane_ui.rs`, `pane_ops.rs`, `overview_plane.rs`, `workbench_host.rs` middlenet-availability check) — these are not routing *decisions* (they're capability / render-path checks), but eliminating them would further reduce Risk 8.4. Deferred to a follow-on "viewer-id type hardening" plan rather than included in this scope.
5. ⚠️ Settings and debug surfaces describe backend choice in `verso` terms (PR 5). The `verso_route` field is now available on `WorkbenchNodeViewerSummary`; actually rendering engine/reason/owner in the UI is a UX-layer follow-on.
6. ✅ Terminology/docs no longer describe Verso primarily as a mod (PR 6).

#### Follow-on work (not part of this plan)

- Reshape `ViewerRoutingDecision.viewer_id: &'static str` into a typed handle (Risk 8.5) — requires an audit of every `== "viewer:*"` site and a coordinated flip. Appropriate as a dedicated plan after this one archives.
- Thread `VersoResolvedRoute` through `NodePaneState` so `resolved_viewer_id` can be replaced by the structured route.
- Render `verso_route.reason()` / `.owner()` in the workbench debug overview pane (UX work).
- "Compat mode" introduction (per §Phase 4): a per-node setting that routes through `WebEnginePreference::Wry`. New feature, separate plan.

**Plan status: complete. Archived 2026-04-21.**
