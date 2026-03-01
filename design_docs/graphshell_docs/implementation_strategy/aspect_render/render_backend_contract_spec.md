# Render Backend Contract Spec

**Date**: 2026-03-01
**Status**: Canonical interaction contract
**Priority**: Active — C+F policy in effect

**Related**:
- `ASPECT_RENDER.md`
- `frame_assembly_and_compositor_spec.md`
- `../../TERMINOLOGY.md` — `CompositorAdapter`, `TileRenderMode`, `BackendContentBridgeMode`
- `../2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`
- `../2026-03-01_webrender_readiness_gate_feature_guardrails.md`
- `../research/2026-03-01_webrender_wgpu_renderer_research.md`
- `2026-03-01_webrender_wgpu_renderer_implementation_plan.md`
- `2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the render backend abstraction layer in
Graphshell: what the backend boundary owns, what crosses it, what must never cross it, and
what the active and planned backend implementations must satisfy.

It covers:

- the backend bridge mode enum and selection policy,
- the capability probe contract,
- the content callback and texture handoff interfaces,
- GL state isolation requirements (Glow path),
- wgpu texture handoff requirements (wgpu path),
- diagnostics obligations per mode,
- fallback routing rules,
- Glow retirement conditions,
- acceptance criteria.

This spec covers the **backend abstraction contract**. Pass ordering, compositor pass
structure, and `TileRenderMode` dispatch are covered in `frame_assembly_and_compositor_spec.md`.
The phased execution plan for the wgpu renderer is in
`2026-03-01_webrender_wgpu_renderer_implementation_plan.md`.

---

## 2. Backend Ownership Boundary

### 2.1 What the backend boundary owns

- the `render_backend` module is the sole ownership boundary for content-bridge mode
  selection — no other module may select or switch the active backend mode,
- the `BackendContentBridgeMode` enum and its variants,
- the `BackendContentBridgeCapabilities` capability probe,
- the `BackendCallbackFn` / `BackendGraphicsContext` type aliases,
- all GL state save/restore operations on the Glow path,
- all wgpu texture handoff operations on the wgpu path,
- the `UiRenderBackend` type alias (egui rendering backend handle),
- diagnostics channel emission for bridge-mode selection, capability results, and
  per-frame handoff timing.

### 2.2 What must never cross the backend boundary

- direct `egui_glow::CallbackFn` or `glow::Context` references in workbench or UI modules
  — those are backend implementation details, not embedder APIs,
- direct `wgpu::Device`, `wgpu::Queue`, or `wgpu::Texture` references in workbench or UI
  modules — the backend boundary mediates all GPU resource exposure,
- backend selection logic in `CompositorAdapter`, `tile_compositor`, or any non-backend
  module — bridge mode is always resolved through `render_backend`.

---

## 3. Backend Bridge Mode Enum

```
BackendContentBridgeMode =
  | GlowCallback
    -- current production path; egui_glow CallbackFn; GL state must be
       saved/restored by CompositorAdapter around every content callback
  | WgpuPreferredFallbackGlowCallback
    -- future path; wgpu texture handoff is primary; GlowCallback activates
       when wgpu interop capability is absent
```

### 3.1 Mode selection policy

- `active_backend_content_bridge_policy()` is the sole function that determines the active
  mode at runtime.
- The function currently returns `GlowCallback` unconditionally (Glow remains the
  production composition path for the current milestone).
- The function will return `WgpuPreferredFallbackGlowCallback` only after readiness gates
  G1–G5 in the readiness gate document are all closed with linked tracker evidence.
- No code outside `render_backend` may call `active_backend_content_bridge_policy()` and
  then act on the result directly — all callers must go through the bridge selection helpers
  exported by `render_backend`.

### 3.2 Mode invariants

**Invariant**: The active mode is determined once per app startup (or once per device
reinit). It is not recalculated per-frame or per-tile. Mode changes require a documented
reinit or restart path.

**Invariant**: `WgpuPreferredFallbackGlowCallback` is not a permanent dual-path mode. It
is a migration bridge. The Glow path within `WgpuPreferredFallbackGlowCallback` must be
retired once wgpu + fallback make it redundant for all supported targets.

---

## 4. Capability Probe Contract

`BackendContentBridgeCapabilities` is a struct that holds the result of probing whether
the runtime environment supports the preferred backend mode.

### 4.1 Probe interface

```
BackendContentBridgeCapabilities {
    wgpu_interop_available: bool,
    -- true iff: wgpu device initialized, Servo WebRender wgpu backend active,
    --           shared device handoff succeeded, zero-copy texture path available
    glow_fallback_available: bool,
    -- true iff: egui_glow context initialized and usable (expected: always true
    --           on any platform Graphshell supports)
    probe_diagnostics_channel: DiagnosticChannelId,
    -- the channel on which probe results are emitted
}
```

### 4.2 Probe timing

- The capability probe runs once at backend initialization, before the first frame.
- The probe result is cached for the session lifetime.
- If the environment changes (e.g. GPU device loss + reinit), the probe re-runs and the
  result is re-evaluated.

### 4.3 Probe invariants

**Invariant**: The probe must never panic. Capability unavailability is a valid result, not
an error.

**Invariant**: The probe result for `wgpu_interop_available` must accurately reflect
whether the zero-copy wgpu texture handoff path works for the current device and platform.
Returning `true` when the path would fail at frame time is a correctness bug.

---

## 5. Glow Path Contract (Current Production)

### 5.1 Content callback interface

On the Glow path, the `CompositorAdapter` invokes Servo's `render_to_parent` callback
inside an `egui::PaintCallback` with an `egui_glow::CallbackFn`.

The `render_backend` module owns the `BackendCallbackFn` type alias. No caller outside
`render_backend` or `compositor_adapter` may construct or invoke a raw `egui_glow::CallbackFn`.

### 5.2 GL state isolation contract

Before invoking the content callback:

- The current scissor box, viewport rect, blend enable, active texture unit, and bound
  framebuffer are saved by `CompositorAdapter`.

After the content callback returns:

- All saved GL state is restored to exactly the pre-callback values.
- If restoration fails for any reason, `CHANNEL_COMPOSITOR_GL_STATE_VIOLATION` is emitted
  at `Error` severity and the frame is completed with a fallback render path.

**Invariant**: The content callback (Servo `render_to_parent`) must not permanently change
any GL state that was not owned by the callback's render scope. Transient state changes
during rendering are acceptable; leaked state after return is a correctness bug.

**Invariant**: GL state save/restore must cover at minimum: scissor box, viewport,
blend enable, active texture unit, bound framebuffer. The chaos mode diagnostics gate
verifies these six invariants in diagnostics-enabled builds.

### 5.3 Chaos mode

In diagnostics-gated builds, `CompositorAdapter` operates in chaos mode:

- GL state is sampled before and after every content callback.
- Any deviation between pre- and post-callback state that was not explicitly allowed by
  the compositor contract is emitted as `CHANNEL_COMPOSITOR_CHAOS_FAIL` at `Error` severity.
- Clean passes emit `CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS` at `Info` severity.

Chaos mode must not be active in production builds (it adds per-callback GPU readback cost).

---

## 6. wgpu Path Contract (Planned — Behind Readiness Gates)

This section defines the contract that the wgpu path must satisfy when it becomes active.
It is not yet production policy. The wgpu path is gated by readiness gates G1–G5.

### 6.1 Compositor output texture interface

On the wgpu path, WebRender's compositor output is a `wgpu::Texture` owned by the shared
device. The `render_backend` module exposes:

```
CompositorOutputTexture {
    view: wgpu::TextureView,
    dimensions: (u32, u32),
    format: wgpu::TextureFormat,
    generation: u64,  -- increments every frame; used to detect stale references
}
```

The `CompositorAdapter`, on the wgpu path, binds `view` directly into the `egui_wgpu`
paint callback without a copy. The GL save/restore machinery is not required (wgpu command
encoder scoping provides structural isolation).

### 6.2 Shared device ownership

- Graphshell owns the `wgpu::Instance`, `Adapter`, `Device`, and `Queue`.
- `egui_wgpu::Renderer` receives the `wgpu::Device` reference at initialization.
- Servo/WebRender receives the same `wgpu::Device` reference at initialization.
- All GPU resources allocated by WebRender's wgpu renderer are allocated on the shared
  device.

**Invariant**: There must be exactly one `wgpu::Device` active in any Graphshell session.
Two devices on the same process are a resource management hazard. If shared device
initialization fails, the wgpu path falls back to Glow via `glow_fallback_available`.

### 6.3 Texture pool contract

The backend maintains a pool of pre-allocated compositor output textures to avoid
per-frame allocation:

- Pool size: at least 2 textures per active `CompositedTexture` tile (double-buffering).
- On tile resize: the pool entry is re-created at the new dimensions; frames during
  re-creation use a placeholder tile.
- `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_HIT` emitted at `Info` when pool rotation succeeds.
- `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_MISS` emitted at `Warn` when a new allocation is
  required mid-session (allocation succeeded but was outside the pool budget).

### 6.4 wgpu isolation invariants

The wgpu path provides structural isolation by construction:

- Each WebRender frame is encoded in its own `wgpu::CommandEncoder`.
- Each Graphshell egui_wgpu frame is encoded in its own `wgpu::CommandEncoder`.
- The two encoders do not share mutable GPU state.
- Chaos mode on the wgpu path verifies that no bind group or pipeline state set inside
  WebRender's encoder is visible inside Graphshell's encoder.

---

## 7. Diagnostics Obligations

All channels follow the Graphshell diagnostics channel schema.

### 7.1 Backend selection channels

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_BACKEND_MODE_SELECTED` | `Info` | Active bridge mode determined at startup |
| `CHANNEL_BACKEND_CAPABILITY_PROBE` | `Info` | Capability probe result recorded |
| `CHANNEL_BACKEND_WGPU_INTEROP_UNAVAILABLE` | `Warn` | wgpu interop probed as unavailable; Glow fallback active |

### 7.2 Glow path channels

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_COMPOSITOR_GL_STATE_VIOLATION` | `Error` | GL state leaked across content callback |
| `CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS` | `Info` | Chaos mode: no state violation detected |
| `CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL` | `Error` | Chaos mode: state violation detected |
| `CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE` | `Info` | Sampled Glow content callback latency in microseconds |

### 7.3 wgpu path channels (active when wgpu path selected)

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_COMPOSITOR_WGPU_HANDOFF_US_SAMPLE` | `Info` | Sampled wgpu texture handoff latency in microseconds |
| `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_HIT` | `Info` | Pool texture rotation succeeded |
| `CHANNEL_COMPOSITOR_WGPU_TEXTURE_POOL_MISS` | `Warn` | Pool allocation was required outside budget |

**Invariant**: Equivalent diagnostic coverage must exist on both paths before the wgpu path
is promoted to production. Parity is a readiness gate (G5).

---

## 8. Fallback Routing Rules

| Scenario | Active mode | Fallback result |
|----------|-------------|----------------|
| wgpu interop probe returns false | `WgpuPreferredFallbackGlowCallback` | Glow path activates; `CHANNEL_BACKEND_WGPU_INTEROP_UNAVAILABLE` emitted |
| GL context lost | `GlowCallback` | Frame skipped; `CHANNEL_COMPOSITOR_GL_STATE_VIOLATION` emitted; recovery per GPU surface lifecycle spec |
| wgpu device lost | `WgpuPreferredFallbackGlowCallback` | Glow path activates if available; `Error` diagnostic emitted; reinit attempted |
| Both paths unavailable | any | Compositor output is `Placeholder`; `Error` diagnostic emitted |

**Invariant**: A fallback activation must never be silent. At minimum one `Warn` or `Error`
diagnostic must be emitted each time the primary path degrades to a fallback.

---

## 9. Glow Retirement Conditions

The Glow path may be retired (removed from the production codebase) only when all of the
following are true, each with linked tracker evidence:

1. Compositor replay diagnostics parity: wgpu path emits equivalent diagnostic coverage to
   the Glow baseline on the same compositor scenarios.
2. No open stabilization regressions tied to pass-order, callback-state isolation, or
   overlay affordance visibility that are Glow-path-specific.
3. Fallback path behavior validated in at least one non-interop environment (e.g. a device
   where `wgpu_interop_available` returns false).
4. All required pass-contract scenarios covered by wgpu-primary + fallback-safe paths with
   tracker-linked evidence.
5. One full release cycle on `WgpuPreferredFallbackGlowCallback` with no Glow-path
   activations reported from the production distribution.

Until all five conditions are met, the Glow path remains a first-class code path, not a
deprecated compatibility shim.

---

## 10. Feature Guardrails (Effective for All New Feature Work)

Any new feature slice touching rendering or composition must comply with:

1. **No new renderer-specific coupling in UI/workflow code** — feature code must consume
   backend contracts, not backend internals (no direct `glow::Context` or `wgpu::Device`
   outside `render_backend`).
2. **Bridge metadata preservation** — any slice touching content-pass wiring must preserve
   bridge-path and bridge-mode diagnostic observability.
3. **Fallback-safe behavior** — new behavior must define what happens when the preferred
   rendering capability is unavailable.
4. **Receipt-linked evidence** — migration-adjacent feature slices must post tracker
   evidence proving guardrail compliance.

---

## 11. Acceptance Criteria

1. `render_backend` is the sole module that selects or switches backend bridge mode.
2. No `egui_glow` or `wgpu` types escape the backend boundary into workbench or UI modules.
3. GL state save/restore covers scissor, viewport, blend, active texture unit, and bound
   framebuffer on the Glow path.
4. Capability probe accurately reflects wgpu interop availability; false positives are
   correctness bugs.
5. All diagnostics channels defined in §7 are registered and emitting.
6. Fallback activations are never silent.
7. Glow path retirement requires all five conditions in §9 with linked evidence.
8. Feature guardrails in §10 are enforced for all new render-adjacent slices.
