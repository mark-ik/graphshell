# Wry Composited Texture Feasibility Spike

**Date**: 2026-03-28  
**Status**: Active spike plan  
**Priority**: Windows-first feasibility check

**Related**:

- `wry_integration_spec.md`
- `VIEWER.md`
- `viewer_presentation_and_fallback_spec.md`
- `../aspect_render/frame_assembly_and_compositor_spec.md`
- `../aspect_render/2026-03-27_egui_retained_state_efficiency_and_physics_worker_evaluation_plan.md`

---

## 1. Why This Is A Spike

Graphshell already has a healthy composited viewer path for Servo and a healthy
native-overlay path for Wry. What it does **not** have is proof that the Wry
stack can produce reliable offscreen frames with acceptable latency, damage
behavior, and texture upload characteristics.

That makes "true Wry composited texture rendering" a **feasibility question**
first and a productization task second.

The spike should therefore answer:

1. Can we get frames out of the Windows Wry/WebView2 stack without abusing the
   overlay path?
2. Can those frames be uploaded into the existing compositor callback path at an
   acceptable cadence?
3. Is the result stable enough to justify a first shipped Windows-only slice?

If the answer to any of those is "not really", we should stop before threading
partial assumptions through the workbench runtime.

---

## 2. Current State

Today the Wry runtime is still overlay-shaped:

- `WryManager` creates native child windows and syncs bounds/visibility.
- `WryViewer` only models overlay behavior.
- `TileRenderMode::NativeOverlay` is the only realized Wry path in practice.
- `CompositedTexture` exists in contracts and enums, but there is no frame
  capture bridge.

This means the missing piece is not a small render-pass patch. It is a
frame-source implementation and all of the policy/telemetry that goes with it.

---

## 3. Recommended Spike Scope

### 3.1 Platform

Windows only.

Rationale:

- WebView2 is the primary current platform.
- It is the most plausible first place to obtain an offscreen/snapshot path.
- A positive Windows result gives us a realistic product wedge even if macOS and
  Linux lag.

### 3.2 Non-goals

- Do not promise macOS/Linux parity in this spike.
- Do not wire persistent settings behavior beyond feature/probe visibility.
- Do not replace the existing native-overlay path.
- Do not generalize the compositor contract until we have a working frame
  producer.

### 3.3 Success Criteria

The spike is successful if all of the following are true on Windows:

1. A Wry-backed pane can produce an RGBA frame or GPU-readable texture without
   requiring a native overlay child window.
2. That frame can be registered through the existing composited content pass.
3. Resize / navigation / first-frame behavior is predictable enough to write
   deterministic diagnostics and tests around.
4. We can characterize likely frame cadence and upload cost.

---

## 4. Suggested Technical Shape

### 4.1 Add A Frame-Source Boundary First

Before implementing capture, split the Wry runtime conceptually into:

- **overlay control**
  - create child webview
  - sync bounds
  - hide/show
- **frame source**
  - request/observe a new frame
  - expose latest frame metadata
  - surface unsupported/not-ready/error states

This prevents the current overlay manager from becoming an accidental dumping
ground for two unrelated rendering models.

Suggested seam:

```rust
pub enum WryFrameAvailability {
    Unsupported,
    Pending,
    Ready,
    Failed,
}

pub struct WryFrameMetadata {
    pub width: u32,
    pub height: u32,
    pub revision: u64,
}
```

The spike does not need a final trait design, but it should create an obvious
home for frame production.

### 4.2 Keep The Compositor Contract Reused

Do not invent a second composition path.

The spike should aim to feed the existing compositor adapter boundary used for
Servo offscreen composition:

- prepare target size
- paint/capture the content
- register the content callback
- let the tile compositor treat it like another composited source

If Wry capture requires radically different assumptions from that path, that is
important spike output in itself.

---

## 5. Execution Order

### Phase A — Capability Probe

- identify the actual Windows capture mechanism we can reach from the current
  Wry/WebView2 integration
- verify whether it yields CPU-readable pixels, shared surfaces, or only a
  snapshot API
- document constraints: alpha, scaling, throttling, event cadence

### Phase B — Runtime Skeleton

- add platform support reporting for composited Wry capture
- add placeholder frame-source status types
- add diagnostics/logging surfaces for "unsupported / pending / failed / ready"

### Phase C — One-Pane Headed Prototype

- attach one Wry viewer in prototype composited mode
- capture frames
- register them through the compositor callback path
- verify resize, occlusion, navigation, and first-frame behavior

### Phase D — Decision

Possible outcomes:

- **Go**: Windows composited Wry is viable; proceed with a guarded implementation
  slice.
- **Partial go**: viable only for specific scenarios; keep overlay as default and
  expose composited mode as experimental.
- **No-go**: keep Wry overlay-only and stop investing in texture mode for now.

---

## 6. Main Risks To Watch

- frame capture exists but is too slow for interactive tiles
- snapshot semantics do not track scrolling/video well enough
- texture upload overhead erases any product value
- focus/input behavior diverges badly from both overlay Wry and composited Servo
- WebView2 capture APIs require a deeper integration surface than `wry` exposes

The last point is especially important: if the required capability lives below
what the current `wry` crate exposes comfortably, the true decision may become
"custom Windows integration" vs "no composited Wry", not merely "finish the
current scaffold."

---

## 7. Recommendation

Proceed with a Windows-only feasibility spike, not a cross-platform product
implementation.

The best next code slice after this document is:

1. support reporting in the Wry runtime for composited capture availability
2. a small frame-source skeleton
3. a headed Windows prototype that attempts to feed one captured frame into the
   compositor

Anything larger than that before capability proof would be premature.
