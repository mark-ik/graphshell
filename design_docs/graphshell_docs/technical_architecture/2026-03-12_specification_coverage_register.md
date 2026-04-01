<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Specification Coverage Register (2026-03-12)

**Status**: Active / Canonical reference register

**Purpose**: Account for the internal and external specifications that govern current and planned Graphshell features. Identify where existing standards are authoritative, where Graphshell must define its own internal specification, and where the codebase is currently underspecified.

**Companion docs**:

- `ARCHITECTURAL_OVERVIEW.md`
- `implementation_strategy/PLANNING_REGISTER.md`
- `implementation_strategy/system/system_architecture_spec.md`
- `implementation_strategy/system/2026-03-12_architectural_inconsistency_register.md`

---

## 1. Policy

Graphshell should prefer **external normative standards** whenever they specify the behavior we want.

Graphshell should define **internal specifications** only when:

1. no credible external standard exists for the behavior,
2. multiple external standards must be projected into one Graphshell-specific model,
3. Graphshell intentionally abstracts, constrains, or deviates from an external standard,
4. Graphshell behavior is entirely product-specific and Graphshell is the only source of truth.

### 1.1 External normative standards

Use external standards as:

- the normative source for browser/content behavior,
- terminology discipline,
- a guardrail for host/embedder behavior and interoperability.

### 1.2 Internal specifications

Internal specs should define:

- Graphshell-specific semantics,
- ownership and mutation boundaries,
- invariants, ordering, fallback, and diagnostics,
- deliberate deviations from external standards.

### 1.3 “Graphshell is the only source of truth”

Mark a feature family this way when:

- there is no relevant external standard,
- the semantics are primarily product/workbench/runtime-specific,
- interoperability is not the main concern; internal coherence is.

---

## 2. External Standards Families Already Relevant

| Standards family | Why it matters here |
| --- | --- |
| WHATWG (URL, HTML, DOM, Fetch, MIME Sniffing, Storage) | Browser/content behavior, URL parsing, document assumptions, MIME-driven viewer behavior |
| HTTP semantics | request/response, redirects, headers, submission behavior |
| ARIA / WCAG / accessibility semantics | accessibility tree projection, command surfaces, focus semantics, user-facing status/state annotations |
| Unicode / emoji / text semantics | text shaping, label truncation, emoji badges, icon search |
| SVG / PNG / image format semantics | icons, thumbnails, previews, image viewers |
| OpenGL / graphics backend contracts | compositor callback behavior, render pass guarantees, backend isolation |
| Gamepad semantics | input sign conventions, navigation behavior |
| Nostr NIPs | relay protocol behavior, state/event model, identity, subscriptions |
| UUID / JSON / serialization conventions | persistence, identity, snapshot/export stability |

---

## 3. Coverage Register

### 3.1 Browser/content/model families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| URL parsing / address semantics | WHATWG URL, HTTP URI semantics | `system/2026-03-03_graphshell_address_scheme_implementation_plan.md` | Partial | No | tighten canonical address behavior doc for Graphshell-specific schemes |
| MIME detection / viewer selection | WHATWG MIME Sniffing, media type conventions | `viewer/viewer_presentation_and_fallback_spec.md`, `viewer/universal_content_model_spec.md` | Partial | No | explicit MIME precedence and fallback matrix if not already centralized |
| HTML / DOM / document behavior | WHATWG HTML, DOM | viewer/spec docs, clipping specs | Partial | No | document explicit projection rules from browser semantics into graph semantics |
| Web storage/content persistence semantics | WHATWG Storage | storage subsystem specs | Partial | No | note Graphshell deviations from browser storage semantics explicitly |
| Node semantic tagging / knowledge | none authoritative for Graphshell tag model beyond UDC side references | `graph/semantic_tagging_and_knowledge_spec.md`, `graph/node_badge_and_tagging_spec.md` | Partial | Yes (except UDC semantics) | tag query semantics, long-term content-type badge expansion, future Lucide re-scope only if reauthorized |
| `#clip` and extracted DOM fragments | loosely informed by DOM semantics; no external Graphshell equivalent | `viewer/clipping_and_dom_extraction_spec.md` | Partial | Yes | decide whether `#clip` remains a tag or becomes explicit node kind |

### 3.2 Workbench/session/layout families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Workbench frame/tile interaction | none | `workbench/workbench_frame_tile_interaction_spec.md`, `workbench/pane_chrome_and_promotion_spec.md`, `workbench/pane_presentation_and_locking_spec.md` | Good for UX, weaker for mutation internals | Yes | `tile_view_ops` mutation/invariant spec |
| Tile groups vs frames vs promotion | none | workbench specs + recent plans | Partial | Yes | explicit structural semantics doc for group/frame/promotion operations |
| Frame persistence / restore | serialization conventions only | storage/workbench docs, `workbench/frame_persistence_format_spec.md` | Partial | Yes | keep format/versioning/recovery rules current as frame semantics expand |
| Workspace/session decomposition | none | `2026-03-12_workspace_decomposition_and_renaming_plan.md` | Active | Yes | execution slices for state extraction and naming cleanup |

### 3.3 Input / command / focus families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Action Registry | none | `aspect_command/command_surface_interaction_spec.md`, `system/register/action_registry_contract_spec.md` | Partial | Yes | extend from coarse capability gating into richer context filtering only if the registry contract actually grows |
| Input Registry / keybinding dispatch | gamepad semantics partially external; keyboard semantics mostly platform/runtime | `aspect_input/input_interaction_spec.md`, `system/register/input_registry_spec.md` | Partial | Mostly yes | `input_registry_dispatch_contract_spec.md` or expand registry spec with priority/conflict rules |
| Focus runtime state machine | ARIA/focus concepts relevant, but not sufficient | `subsystem_focus/SUBSYSTEM_FOCUS.md`, `subsystem_focus/focus_and_region_navigation_spec.md`, `2026-03-08_unified_focus_architecture_plan.md`, `subsystem_focus/focus_state_machine_spec.md` | Good | Yes | keep desired-vs-realized and capture/restore behavior aligned with future focus-authority changes |
| Omnibar / toolbar address bar | WHATWG URL, HTTP submission behavior | command/input specs, `aspect_input/toolbar_omnibar_behavior_spec.md` | Good | Mixed | tighten URL/search arbitration if raw-submit heuristics expand beyond the documented contract |
| Command surface parity | accessibility semantics relevant | `aspect_command/command_surface_interaction_spec.md` | Good | Mostly yes | continue to route surface-specific registry semantics into one registry contract |

### 3.4 Render / viewer / embedder families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Compositor / pass contract | OpenGL/backend contracts; viewer behavior informed by external browser/renderers | `aspect_render/frame_assembly_and_compositor_spec.md`, `aspect_render/render_backend_contract_spec.md`, `aspect_render/2026-03-12_compositor_expansion_plan.md` | Good and improving | Mixed | continue hardening overlay policy, semantic invalidation, and backend-boundary docs as the Glow-first compositor grows |
| RunningAppState / EmbedderCore boundary | none | `../../archive_docs/checkpoint_2026-03-22/graphshell_docs/implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md`, `aspect_render/running_app_state_boundary_spec.md` | Good | Yes | maintain the landed Stage 4b boundary without reintroducing callback-side app mutation |
| Semantic event projection from webview/runtime | WHATWG-derived triggers matter | lifecycle specs, `aspect_input/semantic_event_pipeline_spec.md` | Good | Mixed | keep event ordering, responsive-webview semantics, and diagnostics aligned as additional semantic event kinds land |
| Webview backpressure / admission | none | `viewer/node_lifecycle_and_runtime_reconcile_spec.md`, `viewer/webview_backpressure_spec.md` | Good | Yes | keep blocked-state semantics and retry/cooldown policy aligned with runtime/lifecycle changes |
| Viewer fallback and attachment | WHATWG/MIME influences content classes | `viewer/viewer_presentation_and_fallback_spec.md`, `viewer/node_lifecycle_and_runtime_reconcile_spec.md`, `viewer/wry_integration_spec.md` | Good | Mixed | explicit deviation notes where browser/runtime differs from standards behavior |

### 3.4b Graph physics / layout families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Force-directed layout baseline | Fruchterman-Reingold literature | `canvas/layout_behaviors_and_physics_spec.md`, `canvas/2026-02-24_physics_engine_extensibility_plan.md`, `canvas/force_layout_and_barnes_hut_spec.md` | Partial | Mixed | keep upstream FR alignment explicit as tuning/adapter code evolves |
| Barnes-Hut scaling path | Barnes-Hut n-body approximation literature | `canvas/2026-02-24_physics_engine_extensibility_plan.md`, `canvas/force_layout_and_barnes_hut_spec.md` | Planned / partial | Mixed | land concrete selection/quality/performance contract when implementation starts |

### 3.5 History / storage / diagnostics families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Traversal/history semantics | no general browser/workbench standard covers Graphshell graph-history model | `subsystem_history/SUBSYSTEM_HISTORY.md`, `edge_traversal_spec.md`, timeline specs | Good | Yes | continue replay/time-travel implementation docs only |
| Undo/redo mixed snapshot boundary | none | reset and history docs touch it indirectly | Underspecified | Yes | `undo_redo_scope_spec.md` if mixed-scope undo remains intentional |
| Storage / WAL / persistence integrity | JSON/serialization/crypto conventions external; Graphshell model internal | `subsystem_storage/SUBSYSTEM_STORAGE.md`, `storage_and_persistence_integrity_spec.md`, `workbench/frame_persistence_format_spec.md` | Good | Mixed | keep frame format/versioning/recovery rules current as named-frame behavior expands |
| Diagnostics channels / observability | no external standard | `subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md`, diagnostics specs | Good | Yes | keep channel and analyzer contracts centralized |

### 3.6 Accessibility / UX semantics families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Accessibility bridge / AccessKit projection | WCAG, ARIA, accessibility semantics | `subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md`, `accessibility_interaction_and_capability_spec.md` | Good and improving | Mixed | finish full WebView subtree fidelity plus broader Graph Reader navigation/action coverage under the canonical UxTree -> AccessKit ownership path |
| UX semantic tree / probes / scenarios | none externally define Graphshell UX tree | `subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md`, `ux_tree_and_probe_spec.md`, `ux_event_dispatch_spec.md`, `ux_scenario_and_harness_spec.md` | Good | Yes | keep as Graphshell-only canonical source of truth |
| Badge/accessibility projection | WCAG/ARIA external; Graphshell badge semantics internal | node badge plan, compositor expansion plan, `subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` | Partial / active | Mixed | finish bridging language and authority boundaries through the canonical UX/accessibility projection layer rather than parallel badge-specific semantics |

### 3.7 Protocol / identity / sync families

| Area | External standards | Existing internal specs | Coverage status | Graphshell-only source of truth? | Prospective internal spec work |
| --- | --- | --- | --- | --- | --- |
| Nostr runtime / registry | Nostr NIPs | `system/2026-03-10_nostr_nip_completion_plan.md`, `system/register/nostr_core_registry_spec.md`, `system/register/nostr_runtime_behavior_spec.md` | Partial | No for protocol, yes for graph projection/runtime ownership | keep protocol/runtime split explicit as graph-facing consumers land |
| Verse / coop / graph sync | protocol partially Graphshell-specific | `verso_docs/implementation_strategy/coop_session_spec.md`, register/runtime specs | Partial | Largely yes | keep Graphshell protocol semantics explicit where no external standard exists |
| Identity registry | cryptographic conventions external; Graphshell identity projection internal | identity registry specs | Partial | Mixed | document canonical identity ownership vs protocol adapters |

---

## 4. Modules/Features Still Underspecified

These are the highest-value current gaps, given the code and docs today.

| Module / feature | Why underspecified | Proposed prospective spec |
| --- | --- | --- |
| Input Registry dispatch details | keybinding conflict resolution and action-surface routing are still not fully centralized as a contract | `input_registry_dispatch_contract_spec.md` |
| Undo/redo mixed snapshot boundary | domain, session, and UI targeting still cross one history boundary without a canonical layered contract | `undo_redo_scope_spec.md` |
| `tile_view_ops.rs` mutation invariants | the current interaction spec covers behavior well, but container mutation invariants and repair semantics still deserve a dedicated invariant-first spec | `tile_tree_mutation_invariant_spec.md` |
| Badge-to-accessibility authority boundary | compositor affordance output, badge semantics, and a11y projection are now connected, but the exact canonical ownership language is still evolving | extend `subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` or add a dedicated bridging note |

---

## 5. “Graphshell Is the Only Source of Truth” Areas

These areas are expected to remain primarily internally specified because there is no meaningful external standard for the behavior:

- workbench tile/frame/group/promotion semantics
- graph/workbench/focus authority boundaries
- Graphshell traversal/history model
- tile mutation invariants
- frame snapshot/restore behavior
- runtime backpressure policy
- focus capture/restore model
- UX semantic tree, probes, and scenario model
- Graphshell command/action availability model
- Graphshell-specific graph semantic tags and badge semantics

These should not apologize for being internally specified. They are product architecture.

---

## 6. External Standards That Should Be Made More Explicit

The following external standards families should be cited more deliberately in the architecture docs:

1. WHATWG standards for browser/content behavior
2. WCAG + ARIA for accessibility semantics
3. HTTP semantics for request/submission behavior
4. MIME/media-type standards for viewer selection and content interpretation
5. Unicode/emoji behavior for text and badge/icon systems
6. Nostr NIPs for relay/protocol behavior
7. graphics/backend contracts for compositor behavior where applicable

This does **not** mean every doc needs an exhaustive standards appendix. It means feature families that depend on those standards should say so explicitly when the standard is part of the intended behavior.

---

## 7. Recommended Next Documentation Moves

1. Add a short **Normative Standards Policy** section to `ARCHITECTURAL_OVERVIEW.md`.
2. Treat this register as the canonical coverage map.
3. Prioritize remaining internal specs and follow-through in this order:
   - `input_registry_dispatch_contract_spec.md`
   - `undo_redo_scope_spec.md`
   - `tile_tree_mutation_invariant_spec.md`
   - accessibility bridge follow-through for full WebView subtree fidelity and Graph Reader action routing
4. When a feature plan depends on an external standard, name it explicitly rather than assuming it silently.
5. When Graphshell behavior intentionally diverges from an external standard, document the deviation and the reason.
