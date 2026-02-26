# Architecture / Terminology Alignment Gap Analysis (2026-02-26)

**Status**: Research note (analysis snapshot)
**Scope**: Contradictions and architectural gaps after terminology refactor (`Register` / `Domain` / `Aspect` / `Surface` / `Subsystem` model)
**Related**:
- `design_docs/TERMINOLOGY.md`
- `design_docs/graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md`
- `design_docs/graphshell_docs/technical_architecture/TERM_ARCHITECTURE_DESC.md`
- `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md`

## Summary

The architecture framing is now coherent and substantially improved, but the codebase and some terminology entries still reflect transitional models. The dominant issue is **model-ahead-of-runtime drift**: the docs correctly distinguish `Domain`, `Aspect`, `Surface`, `Pane`, and `Subsystem`, while several runtime paths still use legacy panel/webview semantics and mixed ownership boundaries.

## Confirmed Contradictions

### 1. `Control Panel` mixes aspect and surface semantics

The new terminology introduces an `Aspect`/`Surface` split, but `Control Panel` is still described as both a runtime coordinator and a UI surface in at least one canonical definition.

- `design_docs/TERMINOLOGY.md:70` (`Control Panel` described as "surface/process host")
- `design_docs/TERMINOLOGY.md:117` (`Aspect` definition)
- `design_docs/TERMINOLOGY.md:118` (`Surface` definition)
- `design_docs/graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md:59` (`Control Panel` as async coordinator/process host)
- `design_docs/graphshell_docs/technical_architecture/TERM_ARCHITECTURE_DESC.md:129` (Control Panel behavior treated as an aspect)

**Why this matters**: it weakens the newly established `Aspect` vs `Surface` distinction and creates ambiguity in future runtime/UI ownership discussions.

### 2. Pane/tool intents exist but are not reducer-authoritative

Pane-related intents are defined in `GraphIntent`, but the reducer still no-ops them and GUI code handles some directly.

- `app.rs:1060` (`SplitPane`)
- `app.rs:1068` (`SetPaneView`)
- `app.rs:1076` (`OpenNodeInPane`)
- `app.rs:1084` (`OpenToolPane`)
- `app.rs:2568` (pane/tool intent no-op branch)
- `shell/desktop/ui/gui.rs:1163` (`OpenToolPane` handled in frame loop)

**Why this matters**: it contradicts the architecture principle that intents are the fundamental mutation unit and the reducer is the deterministic state boundary.

### 3. `TileKind::Node` is generic in docs, but runtime helpers still equate it with webview tiles

The terminology and pane model now treat `TileKind::Node(NodePaneState)` as a generic node viewer pane. Runtime helpers still use "webview tile" semantics for all node panes.

- `shell/desktop/workbench/tile_view_ops.rs:45` (`open_or_focus_webview_tile`)
- `shell/desktop/workbench/tile_view_ops.rs:66` (`TileKind::Node(node_key.into())` inserted as webview tile)
- `shell/desktop/workbench/tile_runtime.rs:36` (`has_any_webview_tiles` checks `TileKind::Node(_)`)
- `shell/desktop/workbench/tile_runtime.rs:43` (`all_webview_tile_nodes` from `TileKind::Node`)

**Why this matters**: it blocks clean universal content/viewer semantics and leaks webview assumptions into generic node-pane infrastructure.

### 4. `SignalBus` is correctly documented as planned, but code comments still state it as an implemented Register part

- `design_docs/TERMINOLOGY.md:138` (`SignalBus` planned/equivalent abstraction)
- `shell/desktop/runtime/control_panel.rs:15` (`RegistryRuntime + ControlPanel + SignalBus` in module comment)

**Why this matters**: it creates false certainty when reading runtime code and obscures the actual transitional signal-routing state.

## Architectural Gaps (Not Contradictions, But Material)

### 1. Hybrid authority: legacy panel flags + pane/tool architecture

The runtime is currently bridging legacy panel toggles into tool panes, which is a good migration step but not the final architecture.

- `app.rs:1167` (`show_history_manager`)
- `app.rs:1180` (`show_persistence_panel`)
- `app.rs:1182` (`show_sync_panel`)
- `shell/desktop/ui/gui.rs:1178` (bridge to `ToolPaneState::HistoryManager`)
- `shell/desktop/ui/gui.rs:1185` (bridge to `ToolPaneState::Settings`)

**Gap**: two state authorities (legacy booleans + pane/tile tree).

### 2. Tool-pane architecture is structurally present but behaviorally incomplete

Tool panes are now keyed by `ToolPaneState` and titled correctly, but only diagnostics has real rendering; other tool panes are placeholders.

- `shell/desktop/workbench/tile_behavior.rs:109`-`112` (tool pane titles)
- `shell/desktop/workbench/tile_behavior.rs:383` (diagnostics-specific render path)

**Gap**: subsystem/tool pane framework is in place, but most concrete surfaces are not yet implemented.

### 3. Persistence schema still encodes legacy pane semantics

- `shell/desktop/ui/persistence_ops.rs:23` (`type PaneId = u64`)
- `shell/desktop/ui/persistence_ops.rs:41` (`WebViewNode` pane content)
- `shell/desktop/ui/persistence_ops.rs:288` (`PersistedPaneTile::Diagnostic` special case)

**Gap**: storage/history subsystem guarantees will be harder to enforce while persistence keeps legacy special-case pane encodings.

### 4. Register runtime dispatch is not yet authoritative

- `shell/desktop/runtime/registries/mod.rs:184` (explicit comment that desktop still uses legacy dispatch in places)

**Gap**: docs describe Register-owned provider wiring and routing as canonical; runtime remains partially transitional.

### 5. `ControlPanel` internal globals weaken Register boundaries

- `shell/desktop/runtime/control_panel.rs:36` (`OnceLock` global sync command sender)
- `shell/desktop/runtime/control_panel.rs:42` (`OnceLock` global discovery results)

**Gap**: global mutable routing/state bypasses the intended Register-owned signal/event architecture.

### 6. Accessibility subsystem guarantee framing is ahead of current WebView bridge conformance

The bridge currently drops queued Servo accessibility updates due version mismatch.

- `shell/desktop/ui/gui.rs:1403`-`1410` (temporary fallback, dropped pending batches)

**Gap**: architecture correctly frames `accessibility` as a subsystem with degradation/conformance, but a key path remains degraded.

### 7. Capability declarations / conformance model is defined but not encoded in runtime descriptor types

Terminology defines folded surface capability declarations, but viewer/workbench surface registries do not yet carry subsystem capability/conformance metadata.

- `design_docs/TERMINOLOGY.md:162`
- `registries/atomic/viewer.rs`
- `registries/domain/layout/viewer_surface.rs`
- `registries/domain/layout/workbench_surface.rs`

**Gap**: conformance remains descriptive/documentary rather than runtime-authoritative.

## Structural Doc Risks

### Duplicate canonical definitions of `The Register`

`TERMINOLOGY.md` contains two `The Register` definitions (interface component and registry architecture sections). They are aligned today, but duplication increases drift risk.

- `design_docs/TERMINOLOGY.md:73`
- `design_docs/TERMINOLOGY.md:107`

## Recommended Remediation Order (High Leverage)

1. **Split `Control Panel` into aspect vs surface language** in `TERMINOLOGY.md` and `SYSTEM_REGISTER.md`.
2. **Define `Signal` vs `Intent` vs direct call routing rules** in `SYSTEM_REGISTER.md` and use them to guide `#81` / `#82`.
3. **Make pane/tool intents reducer-authoritative** (or explicitly document a separate workbench mutation authority if retained).
4. **Refactor node-pane runtime helpers away from webview naming/assumptions** (`tile_runtime.rs`, `tile_view_ops.rs`).
5. **Normalize persistence schema terminology and special cases** away from `WebViewNode` / `Diagnostic`-specific variants.
6. **Add capability/conformance fields to surface/viewer descriptors** and link subsystem diagnostics/validation to them.
7. **Replace `SignalBus` implementation claims in code comments** with transitional wording until the routing layer lands.

## Why This Research Matters

The terminology work succeeded: the architecture now has a coherent vocabulary. The remaining work is to make runtime authority and persistence/schema semantics conform to that vocabulary. The highest-risk failure mode is no longer naming confusion; it is **inconsistent enforcement of the intended boundaries** (reducer authority, Register authority, and generic node-pane semantics).
