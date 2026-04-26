<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Servo-into-Verso Lane (2026-04-25)

**Status**: Active execution plan — sliced S1 → S5
**Lane**: Move Servo's role behind a `verso/servo-engine` feature so
graphshell can be selectively compiled with or without Servo, mirroring
the 2026-04-25 wry-into-verso refactor.

**Related**:

- [VERSO_AS_PEER.md](../../../verso_docs/technical_architecture/VERSO_AS_PEER.md) —
  the spec verso has been working toward; calls out Servo (as
  `viewer:webview`) and wry (as `viewer:wry`) as the two engines
  verso owns.
- [Iced host migration execution plan](2026-04-14_iced_host_migration_execution_plan.md) —
  Phase A2 (wry impl → verso) landed 2026-04-25, establishing the
  pattern this plan follows for Servo.
- [Iced content-surface scoping](2026-04-24_iced_content_surface_scoping.md) —
  §0 platform-tier framing: native = Servo, mobile/web =
  middlenet/wry. Servo gating is the prerequisite for non-native
  builds.

---

## 1. Goal

graphshell main becomes selectively compileable across three
independent engine axes:

| Cargo feature | Engine | Default? |
|---|---|---|
| `verso/servo-engine` (via graphshell `servo-engine`) | `viewer:webview` (Servo, texture mode) | yes (matches today) |
| `verso/wry-engine` (via graphshell `wry`) | `viewer:wry` (system WebView, overlay mode) | yes (matches today) |
| middlenet (always-on, lightweight) | `viewer:middlenet` lanes | yes |

The three are independent; **a build without `servo-engine` should
still compile and run** — chrome + middlenet + wry only. This unlocks:

- Mobile / WASM target paths (Servo isn't viable there)
- Faster CI iteration loops for chrome-focused work
- A test surface for graceful degradation when one engine is unavailable

---

## 2. Today's Reality

**Workspace crates** ✅ — all servo-free:

- `graphshell-core`, `graphshell-runtime`, `graph-canvas`, `graph-tree`,
  `middlenet-*`, `verso`, `verso-host`, `iced-middlenet-viewer`,
  `iced-graph-canvas-viewer`, `iced-wry-viewer`. None depend on the
  `servo` crate. Only docstring mentions of `servo::WebViewId` exist
  in `graphshell-core/src/content.rs`.

**Graphshell main** ❌ — Servo is unconditional:

- [Cargo.toml:142](../../../Cargo.toml) — `servo = { path = "...", default-features = false }`
  is a non-optional direct dep.
- ~75 `.rs` files in graphshell main directly `use servo::*` types.
- Many graphshell features cascade into Servo features
  (`gamepad = ["servo/gamepad"]`, `webgpu = ["servo/webgpu"]`, lines
  70–87 of Cargo.toml).

So gating Servo is the same magnitude of refactor as the iced host
migration was — broad mechanical sweep + a handful of architectural
decisions about what graphshell-without-Servo does.

---

## 3. Sliced Execution Plan

### S1 — `verso/servo-engine` feature scaffold

Mirror the wry-engine pattern from Phase A1 (2026-04-25):

- Add `servo-engine = ["dep:servo", ...]` to verso's `[features]`.
- Add `servo` Cargo dep to verso, `optional = true`, identical
  features to graphshell main today.
- New `verso::servo_engine` module re-exports the `servo` crate so
  downstream consumers depend on `verso/servo-engine` rather than
  `servo` directly.
- Verso compiles with the feature on; nothing in graphshell main
  changes yet.

**Receipt**: `cargo check -p verso --features servo-engine` clean.

### S2 — Graphshell main gates `servo::*` behind `servo-engine`

The bulk of the mechanical work, split into S2a (Cargo wiring,
done) and S2b (file-level sweep, in progress).

#### S2a — Cargo wiring (✅ landed 2026-04-25)

- Both `servo` deps in `Cargo.toml` (cross-platform line 142, Windows
  target-conditional line 263) marked `optional = true`.
- New feature `servo-engine = ["dep:servo", "verso/servo-engine"]`.
- All previously-forwarded servo features now require `servo-engine`
  before forwarding into the `servo` crate: `gamepad`, `webgpu`,
  `webxr`, `js_jit`, `crown`, `debugmozjs`, `jitspew`,
  `js_backtrace`, `vello`, `webgl_backtrace`, `tracing`,
  `media-gstreamer`, `native-bluetooth`, `profilemozjs`,
  `refcell_backtrace`.
- `default` feature set keeps `servo-engine` on, so existing builds
  are bit-identical to pre-S2a behavior.

**Receipt (S2a)**: `cargo check` (default features) clean — verified
2026-04-25, completes in 4m 17s, no regressions, two pre-existing
egui deprecation warnings only.

#### S2b — File-level sweep (🚧 module-level pass landed; body-level cascades remain)

**Module-level pass (2026-04-25 evening)**: gated Servo-coupled
submodules at their parent `mod.rs` level. The structural cuts:

- `shell/desktop/mod.rs`: gate `host` + `render_backend` (entirely
  Servo-coupled).
- `shell/desktop/lifecycle/mod.rs`: gate `lifecycle_reconcile`,
  `semantic_event_pipeline`, `webview_backpressure`,
  `webview_controller`, `webview_status_sync` (kept
  `lifecycle_intents` open).
- `shell/desktop/workbench/mod.rs`: gate `compositor_adapter`,
  `tile_render_pass`, `tile_runtime`, `tile_view_ops`, +
  consumer cascades: `graph_tree_dual_write`, `tile_behavior`,
  `tile_compositor`, `tile_invariants`, `tile_post_render`,
  `graph_tree_projection`, `semantic_tabs`, `ux_probes`.
- `shell/desktop/runtime/protocols/mod.rs`: gate `resource`,
  `router`, `servo`, `urlinfo` (kept `registry` open).
- `shell/desktop/runtime/registries/mod.rs`: gate
  `workbench_surface` + `workflow`; `ServoUrl` aliased to
  `url::Url` when servo-engine is off.
- `shell/desktop/ui/mod.rs`: gate `dialog`, `dialog_panels`,
  `egui_host_ports`, `gui`, `gui_frame`, `gui_orchestration`,
  `host_ports`, `nav_targeting`, `persistence_ops`,
  `thumbnail_pipeline`, `toolbar`, `toolbar_routing`,
  `workbench_host`, plus the consumer cascade `gui_state`,
  `finalize_actions`, `graph_search_flow`, `graph_search_ui`,
  `overview_plane`, `shell_layout_pass`. The in-tree iced
  launch path (`iced_app` etc.) now requires both `iced-host`
  AND `servo-engine` (see "iced launch path coupling" below).
- `lib.rs`: gate `mod render` (egui rendering layer);
  `mod parser`, `mod prefs`, `mod resources` behind
  servo-engine; provide a tiny stub `mod prefs { ... }` exposing
  `FileAccessPolicy` so `graph_app.rs` still compiles.
- `shell/desktop/runtime/cli.rs`: split into `main()` (host-neutral
  prelude + iced-host branch + no-servo exit warning) and
  `run_servo_launch_path()` (Servo+egui boot, gated).
- `shell/desktop/runtime/tracing.rs`: gate `from_winit` LogTarget
  impls (use host::event_loop::AppEvent).
- `shell/desktop/runtime/diagnostics.rs`: gate
  compositor_adapter import.
- `panic_hook.rs`: gate `servo::opts` use; SIGSEGV path becomes
  no-op without servo-engine.
- `graph_app.rs`: bug fix — disambiguate `use
  ::graph_cartography::*` (workspace crate, vs. local
  `mod graph_cartography` shim).

**Receipt (module-level pass)**:

- ✅ Default build (`cargo check`, servo-engine + wry on) clean,
  no regressions, ~18s incremental.
- ⏳ `cargo check --no-default-features --features wry`:
  **down to 75 errors** from 142, all body-level cascades.

**iced launch path coupling**: the in-tree iced launch path
(`shell::desktop::ui::iced_app`, `iced_host`, `iced_host_ports`,
etc.) consumes `host_ports::*` traits, `render_backend::Backend*`
types, `compositor_adapter`-re-exported `PortableRect`, and
`servo::WebViewId` directly. Achieving a true no-Servo iced
launch path requires extracting these to host-neutral locations
(graphshell-core for vocab; graphshell-runtime for traits) which
is **S3 architectural work**, not S2b mechanical sweep. For now,
the iced-host launch path inside graphshell main is gated with
`cfg(all(feature = "iced-host", feature = "servo-engine"))`.
The standalone iced demo crates (`crates/iced-{middlenet,graph-canvas,
wry}-viewer`) are unaffected — they remain fully no-Servo and
demonstrate the Phase B portable surface.

**S2c body-level pass (completed 2026-04-25)**:

The deferred no-Servo Wry body-level cascade is now closed. A fresh
S2c baseline after the host-port extraction stood at **74 errors / 24
warnings** for `cargo check --no-default-features --features wry`.
The pass fixed false Servo coupling and added no-Servo shims only where
the build already owned no live Servo state:

- 74 -> 64: `ux_tree` metadata/telemetry imports moved to
  host-neutral sources; Tool-pane match arms made exhaustive when
  `diagnostics` is off.
- 64 -> 56: pure local-file URL/access-policy helpers moved out of
  Servo-gated `tile_behavior` into ungated `workbench::local_file_access`.
- 56 -> 53: `tag_panel` stopped depending on Servo-gated
  `render::semantic_tags` for pure tag label/suggestion helpers.
- 53 -> 41: persisted command-palette action taxonomy now uses
  `graphshell_core::actions`; the no-Servo `prefs` stub gained the
  stylesheet source reader needed by settings persistence.
- 41 -> 32: persisted workspace rename now updates the JSON `name`
  field directly; diagnostics gained no-Servo focus/compositor replay
  and content-budget fallbacks.
- 32 -> 1: `workflow` registry compiled unconditionally; no-Servo
  `workbench_surface` shim backed by the domain registry satisfied the
  `RegistryRuntime` fields.
- 1 -> 0: no-Servo `workbench_surface::dispatch_intent` added as a
  no-op sink matching the Servo-path signature.

Validation receipts:

- ✅ `cargo check --no-default-features --features wry` — clean,
  warnings only (`graphshell` lib generated 24 warnings; 18.54s
  incremental).
- ✅ `cargo check -p graphshell --lib` — clean default-feature guardrail,
  warnings only (`graphshell` lib generated 2 warnings after trimming the
  stale `tile_behavior` re-export; latest incremental 10.72s).

Historical S2b cascade inventory, now resolved by S2c:

- `shell/desktop/runtime/registries/mod.rs` body (~30 refs to
  `WorkbenchSurfaceRegistry`, `WorkflowRegistry`, etc.) — needs
  per-line gating in fields, function signatures, match arms.
- `shell/desktop/runtime/diagnostics.rs` body (`CompositorReplaySample`,
  `replay_samples_snapshot`).
- `app/persistence_facade.rs`, `app/settings_persistence.rs`
  (use `prefs::read_user_stylesheet_source`, `crate::render::*`,
  `ui::persistence_ops`).
- `app/workspace_state.rs` (uses `crate::render::*`).
- `registries/atomic/viewer.rs`, `registries/viewers/{directory,
  image_viewer, middlenet, plaintext}.rs` (5 files: use
  `workbench::tile_behavior`).
- `shell/desktop/workbench/ux_tree.rs` body (gated types in
  function signatures).
- `shell/desktop/ui/tag_panel.rs` (uses `crate::render`).

These were body-level uses of gated types. The S2c fix kept the
Servo/default path on the real modules and used empty/no-op fallbacks
only for no-Servo paths where no Servo producer exists.

#### S2b — Original file-level sweep target (✅ catalogued, ⏳ deferred)

`cargo check --no-default-features --features iced-host,wry` against
post-S2a tree surfaces **141 errors across 58 unique files**
(close to the pre-survey "~75 files" estimate). Inventory below
freezes the sweep target so future sessions can resume mid-sweep
without re-running the full check.

Categories (by gating strategy):

**Cluster A — gate-as-whole-module candidates** (Servo coupling
is structural; entire file is a Servo embedder/compositor adapter):

- `shell/desktop/host/*` (~17 files): `accelerated_gl_media`,
  `embedder`, `event_loop`, `geometry`, `headed_window` (+
  `clip_extraction`, `embedder_controls`, `input_routing`, `xr`),
  `headless_window`, `host_app`, `keyutils`, `running_app_state`
  (+ `webview_delegate`), `webdriver_runtime`, `window` (+
  `projection`, `runtime`).
- `shell/desktop/lifecycle/`: `lifecycle_reconcile`,
  `webview_backpressure`, `webview_controller`,
  `webview_status_sync`, `semantic_event_pipeline`.
- `shell/desktop/render_backend/`: `mod`, `shared_wgpu_context`,
  `wgpu_backend`.
- `shell/desktop/runtime/protocols/`: `resource`, `router`, `servo`,
  `urlinfo` (Servo URL-scheme protocol handlers).
- `shell/desktop/workbench/`: `compositor_adapter`,
  `tile_render_pass`, `tile_runtime`, `tile_view_ops`.
- `shell/desktop/ui/thumbnail_pipeline.rs` (Servo screenshot pipeline).

**Cluster B — partial-gate candidates** (file is host-neutral
overall but pulls Servo types at specific boundaries):

- `shell/desktop/runtime/registries/mod.rs:99` — single import.
- `shell/desktop/runtime/cli.rs:75` — single import in startup path.
- `shell/desktop/ui/dialog.rs`, `dialog_panels.rs`.
- `shell/desktop/ui/egui_host_ports.rs` — egui-host bridge
  (egui being retired; can stay servo-coupled at file level since
  iced-host is the post-retirement path).
- `shell/desktop/ui/iced_host_ports.rs`, `iced_host.rs` — iced-host
  bridge; **must work both with and without `servo-engine`** since
  this is the litmus-test path. Partial gating required.
- `shell/desktop/ui/host_ports.rs` — generic host bridge, partial.
- `shell/desktop/ui/gui.rs` (+ `accessibility`, `gui_frame`,
  `gui_orchestration`, `pre_frame_flow`, `semantic_lifecycle_flow`,
  `toolbar_phase_flow`) — egui-host main loop, similar reasoning to
  egui_host_ports.rs.
- `shell/desktop/ui/nav_targeting.rs`, `persistence_ops.rs`.
- Crate-root: `panic_hook.rs`, `prefs.rs`, `parser.rs`,
  `graph_resources.rs`.

**Sweep approach** (recommendation for next session):

1. Start with Cluster A: gate at the `mod.rs` declaration site
   (e.g., add `#[cfg(feature = "servo-engine")] pub mod host;`
   in `shell/desktop/mod.rs`). One edit per cluster, ~7 mod-level
   edits removes ~40 of 58 file errors at once.
2. Then Cluster B: edit-by-edit gating of import lines and call
   sites, focusing on iced_host* first since those are the path
   that must stay alive without `servo-engine`.
3. Imports become `verso::servo_engine::*` rather than `servo::*`,
   so the route through verso is consistent with the wry pattern
   established in Phase A2.

**Sweep blocker** (surfaced 2026-04-25): the workspace's parallel
`webrender-wgpu` checkout has a compile error in
`webrender_build/src/compiled_artifacts.rs:23` (`unresolved import
crate::glsl`). This blocks `cargo check` from progressing far
enough to surface graphshell-main errors when the cache is cold for
a given feature combo. Next session must either fix or stash that
checkout's working tree before the sweep can iterate. The 58-file
inventory above was captured before the blocker manifested, so the
sweep can proceed against this list without re-running the check.

**Receipt (S2b, deferred)**: `cargo check --no-default-features
--features iced-host,wry` clean (servo-engine off, iced + wry only)
once the sweep lands. `cargo check` with default features
(servo-engine on) must remain clean throughout.

### S3 — `graphshell-without-Servo` runtime architecture

What does the binary actually *do* when `servo-engine` is off?

Decision matrix:

| Subsystem | With `servo-engine` | Without `servo-engine` |
|---|---|---|
| Chrome + canvas + iced/egui host | works | works |
| middlenet content rendering | works | works |
| wry overlay (fullnet) | works (when `wry` feature on) | works (when `wry` feature on) |
| `viewer:webview` (Servo) | works | unavailable; routes to wry / middlenet / unsupported |
| Servo wgpu shared device | works | not constructed |
| Servo accesskit bridge | works | stubbed |
| Webview backpressure | works | reduced to wry-only path |
| Workbench compositor `ViewerSurfaceBacking::NativeRenderingContext` | works | always `None`; callback-fallback or wry-only |

**Key code areas** that need restructuring (not just gating):

1. **`HostCapabilities` defaults**: today `HostCapabilities::default`
   has `supports_servo: false` already (per
   [verso/src/lib.rs:58](../../../crates/verso/src/lib.rs)). The
   live wiring sets it to `true` when graphshell boots Servo.
   Without `servo-engine`, the live wiring stays `false` and verso's
   dispatch routes accordingly. **No breaking change here**.
2. **`ViewerSurfaceRegistry::backing` typing**: currently can be
   `NativeRenderingContext(Rc<dyn RenderingContextCore>)` where
   `RenderingContextCore` is a trait Servo provides. Without
   Servo, the trait still exists (we own the trait? need to verify)
   but no impls are imported. The variant becomes uninhabitable —
   but the enum compiles fine; just no producers.
3. **`shared_wgpu_context.rs`**: holds `servo::wgpu::Device` +
   `servo::wgpu::Queue`. Either gate the whole file behind
   `servo-engine`, or extract the wgpu types into a `verso::wgpu`
   re-export so `servo-engine`-off builds use a stub or the
   `wgpu` crate directly.

For S3 first pass: **gate liberally, document the architectural
follow-ons, don't refactor the trait surface**. Goal is a working
no-servo build, not the cleanest possible no-servo architecture.
The trait extraction (e.g., move `RenderingContextCore` into verso
as a portable trait) is a separable later slice.

**Receipt**: with `servo-engine` off, the binary opens an iced
window with chrome + canvas + wry overlay. Submitting an `https://`
URL falls back to wry (or returns "engine not available" via
verso's existing dispatch) instead of attempting Servo.

### S4 — Startup path gating

- `cli.rs::main()` currently initializes Servo unconditionally.
  Wrap Servo init in `#[cfg(feature = "servo-engine")]`; provide a
  no-servo branch that proceeds to graphshell startup without
  Servo.
- The iced-host launch path (already gated on `iced-host` feature)
  is independent; it'll work with or without `servo-engine`.
- Egui-host launch path needs the same conditional Servo init
  treatment, since today it expects Servo to exist.

**Receipt**: `--no-default-features --features iced-host` runs
`graphshell --iced` against a Servo-free binary. `--no-default-features
--features iced-host,wry` adds wry overlays.

### S5 — Build matrix + documentation

- Add a CI build configuration (or document one if CI isn't
  automated yet): `cargo check --no-default-features --features
  iced-host,wry` should be part of the check matrix to prevent
  regressions where a non-`servo-engine` change breaks the
  no-Servo build.
- Update [PROJECT_DESCRIPTION.md](../../../PROJECT_DESCRIPTION.md)'s
  rendering-architecture paragraph to reflect tri-engine selectivity.
- Update [VERSO_AS_PEER.md](../../../verso_docs/technical_architecture/VERSO_AS_PEER.md)
  to note Servo + wry are both behind verso features, not just
  registered viewers.
- Update the iced-host migration plan with a Phase A2 sibling
  entry for Servo (S2/S3 receipts).

**Receipt**: documentation changes land in the same session; the
"three independent engine axes" picture is canonically captured.

---

## 4. Open Architectural Questions (informs S3)

1. **Does `ViewerSurfaceBacking::NativeRenderingContext` survive
   without Servo?** If we want `servo-engine`-off builds to still
   support some "native rendering context" (e.g., a future
   non-Servo wgpu producer), the `RenderingContextCore` trait
   needs to live in verso (not be re-exported from servo). For
   first-pass, gate the variant entirely — re-introduce when a
   non-Servo producer arrives.
2. **Webview lifecycle vocabulary**: `WebviewBackpressureState` and
   the `webview_backpressure` module assume Servo's webview
   creation cadence. Need to identify what's Servo-specific vs.
   what's host-neutral state machine. Probably gate the whole
   module behind `servo-engine` for first pass.
3. **Accesskit bridge**: Servo provides accesskit tree updates per
   webview. Without Servo, the bridge has no producers but still
   has consumers (chrome accesskit). Stub the producer side.
4. **Shared wgpu device acquisition**: today Servo provides the
   wgpu device that webrender + the compositor share. Without
   Servo, the chrome's iced renderer is the only wgpu consumer
   (plus future iced-graph-canvas-viewer with WebRender — but
   that's webrender-wgpu's wgpu, not Servo's). Need to identify
   who owns the device in non-Servo builds.

---

## 5. Receipts at a glance

Status as of 2026-04-25:

- ✅ `cargo check -p verso --features servo-engine` — clean (S1).
- ✅ `cargo check` (default features) — clean (S2a; servo-engine + wry
  on, no regressions, 4m 17s).
- ✅ `cargo check --no-default-features --features wry` — clean after
  S2c body-level pass; warnings only (24 graphshell warnings).
- ✅ `cargo check --no-default-features --features iced-host,wry` —
  clean as a **library compile** (24 warnings, all unused-import noise,
  0 errors; 1m 54s cold). **Caveat**: the in-tree iced launch path
  (`iced_app`, `iced_host`, `iced_graph_canvas`, `iced_events`,
  `iced_middlenet_viewer`) is still gated on
  `cfg(all(iced-host, servo-engine))` per S3b.1, so this receipt
  proves the iced-host *bridge surface* (`iced_host_ports`,
  `CachedTexture`, runtime ports) compiles without Servo — not that
  the binary launches via iced. Closing the gap (truly launchable
  iced without Servo) is the canonical S3b GraphshellRuntime
  extraction.
- ⏳ `cargo check --no-default-features --features iced-host` — not
  yet attempted (drops `wry` too; expect new errors only if any
  ungated code assumed wry was present).
- ⏳ `cargo check --no-default-features --features
  servo-engine,iced-host` — not yet attempted.
- All matrix entries to be documented post-S5.

**Compile-matrix runner**: [`scripts/dev/engine-feature-matrix.sh`](../../../../scripts/dev/engine-feature-matrix.sh)
(and `.ps1` sibling) runs the three checks above in sequence and emits a
one-line PASS/FAIL summary per combo. Wire this into CI or a pre-push hook
to prevent silent regressions of the no-Servo paths.

---

## 6. Execution log

- **2026-04-25 (S1)**: Added `servo-engine` feature + optional `servo`
  dep + `verso::servo_engine` re-export module to `crates/verso`.
  Verso compiles standalone with the feature on.
- **2026-04-25 (S2a)**: Made graphshell main's `servo` deps (both
  cross-platform line 142 and Windows-target-specific line 263)
  optional. Added `servo-engine = ["dep:servo",
  "verso/servo-engine"]`. Cascaded all 16 servo-forwarded features
  to require `servo-engine` first. Default feature set keeps
  `servo-engine` on; default build verified clean (4m 17s).
- **2026-04-25 (S2b survey)**: Surveyed `cargo check
  --no-default-features --features iced-host,wry` errors;
  catalogued 141 errors across 58 files into Cluster A (whole-module
  gate candidates) and Cluster B (partial-gate candidates). See §3
  S2b for the full inventory. Discovered concurrent
  `webrender-wgpu` working-tree breakage that blocks further
  cargo-check iteration; flagged as sweep prerequisite.
- **2026-04-25 (S2b module-level pass)**: webrender-wgpu blocker
  cleared; ran the module-level gating pass across `lib.rs`,
  `shell/desktop/{mod,lifecycle/mod,workbench/mod,ui/mod,
  runtime/{mod,cli,tracing,diagnostics,registries/mod,protocols/mod}}`,
  plus `panic_hook.rs` and `graph_app.rs`. Down from 142 → 75
  errors against `cargo check --no-default-features --features
  wry`; default build (servo-engine on) remains clean. Remaining
  75 are body-level cascades that S3a (host_ports trait
  extraction) should supersede; deferred to S2c post-S3.
- **2026-04-25 (S3a host-port trait extraction)**: moved the
  host-port trait surface into `graphshell-runtime`:
  `HostInputPort`, `HostSurfacePort`, `HostPaintPort`,
  `HostTexturePort`, `HostAccessibilityPort`, plus
  `BackendViewportInPixels` and the new host-neutral
  `ViewerSurfaceId`. `HostSurfacePort` gained an associated
  `BackendContext` type so iced (`= ()`) and egui (`= glow::Context`)
  can ship without trait-signature churn. Tree-update injection
  was split out into a Servo-specific extension trait
  `ServoAccessibilityInjectionPort` that lives in graphshell-main
  (gated on `servo-engine`) since the egui-host's accesskit anchor
  derivation is `servo::WebViewId`-shaped today; the portable
  `HostAccessibilityPort` retains only `request_focus`. Shell-side
  `host_ports.rs` is now a thin re-export shim, so existing call
  sites work unchanged. `iced_host_ports.rs` no longer imports
  `render_backend` or `compositor_adapter` (it imports from
  graphshell-runtime + graphshell-core directly); the
  type-level painter stubs that did consume those gated modules
  are themselves gated on `servo-engine`. Default build clean.
  No-servo error count holds at 74 (S3a doesn't reduce body-level
  cascade count; that's S2c work). The architectural seam is the
  point: future iced launch path decoupling (S3b) can proceed
  without re-doing port plumbing.
- **2026-04-25 (S3b.1 IcedWgpuContext gate + iced_host_ports
  ungating)**: smaller incremental S3b slice. `IcedWgpuContext`
  (slot for iced-side wgpu device/queue) was Servo-typed because
  its only intended consumer was Servo-produced texture imports;
  gated on `servo-engine` so iced-only builds don't carry the
  Servo wgpu surface. `CachedTexture` relocated from `iced_host.rs`
  into `iced_host_ports.rs` so the ports module has no shell-side
  gated deps; `iced_host_ports` is now ungated from `servo-engine`
  in `ui/mod.rs` and ships under just `iced-host`. The remaining
  iced launch path (`iced_app`, `iced_host`, `iced_graph_canvas`,
  `iced_events`, `iced_middlenet_viewer`) still consumes
  `gui_state::GraphshellRuntime` and stays gated on
  `cfg(all(iced-host, servo-engine))` until the GraphshellRuntime
  extraction (S3b proper) lands.
- **2026-04-25 (S2c body-level no-Servo Wry pass)**: closed the
  `cargo check --no-default-features --features wry` cascade from a
  fresh 74-error baseline to green. Fixes were localized to false
  Servo coupling (`ux_tree`, command-surface telemetry, action
  taxonomy), pure helper relocation (`workbench::local_file_access`,
  tag-panel helpers), JSON/prefs no-Servo fallbacks, diagnostics
  no-Servo placeholders, and a no-Servo `workbench_surface` shim for
  the registry runtime. Final receipts: no-Servo Wry check clean with
  24 graphshell warnings; default `cargo check -p graphshell --lib`
  clean with 2 graphshell warnings after stale re-export cleanup.
- **2026-04-26 (S3b retry/cooldown core extraction)**: continued the
  canonical runtime-crate slice path. `WebviewAttachRetryState` (the
  host-neutral retry/cooldown core named in the
  webview_creation_backpressure audit) moved into
  `graphshell-runtime::webview_backpressure` with a pure
  `min*2^step`-clamp cooldown delay, dropping the `backon` dependency
  from the runtime-side numerics. Shell-side
  `WebviewCreationBackpressureState` now composes the runtime type
  alongside the Servo-typed pending probe and `Instant` deadline —
  matching the audit's recommended split (probe identity + deadline
  stay shell-side because they bind to `WebViewId` and
  `std::time::Instant`). Receipts: graphshell-runtime tests 26 → 33
  pass (8 new tests on the extracted core); shell webview_backpressure
  tests still pass (7); engine-feature matrix all 3/3 PASS. See
  the canonical plan's
  [Source-side audit progress log](../shell/2026-04-24_graphshell_runtime_crate_plan.md#source-side-audit---webview-creation-backpressure-2026-04-25)
  2026-04-26 entry for full details.
- **2026-04-27 (gl_compat gating cascade)**: completed slice 1 of the
  GL-retirement ordering by gating the GL-callback machinery behind
  `gl_compat`, so the wgpu-only build path is now compileable. `glow` is
  now an `optional = true` dep (Cargo.toml:244), activated by
  `gl_compat = ["dep:glow"]`. Gated as `gl_compat`-only:
  `BackendGraphicsContext` / `BackendFramebufferHandle` /
  `BackendParentRenderCallback` (gl_backend.rs); the entire
  `BackendContentBridge*` selection machinery + tests + env-var helpers
  (render_backend/mod.rs); the `custom_pass_from_backend_viewport` /
  `register_custom_paint_callback` stubs (wgpu_backend.rs); the content
  callback registry static + types + accessor + register/unregister/compose
  family (compositor_adapter.rs ~10 functions); the
  `ContentPassPainter::register_content_callback_on_layer` trait method
  and its egui impl; `register_content_callback_from_render_context`,
  `content_callback_from_parent_render`,
  `registered_content_pass_callback`. The
  `cfg(not(feature = "gl_compat"))` variant of
  `run_content_callback_with_guardrails` was retired (had no callers
  without the gated registry). The GL-callback fallback arm in
  `compose_webview_content_pass_with_painter` and the unregister calls
  in the wgpu success path are now conditionally compiled. Two-forked
  `EguiHostPorts: HostSurfacePort` impl: `gl_compat`-on uses
  `BackendContext = BackendGraphicsContext` and forwards to
  `CompositorAdapter::register_content_callback`;
  `gl_compat`-off uses `BackendContext = ()` with no-op
  register/unregister methods (the registry doesn't exist, so callbacks
  are silently dropped). `retire_node_content_resources` and
  `retire_stale_content_resources` skip the registry path under
  no-gl_compat but still clean the native-texture registry.
  **New matrix entry** (slot 4 in
  `scripts/dev/engine-feature-matrix.{sh,ps1}`):
  `--no-default-features --features
  servo-engine,gamepad,js_jit,max_log_level,webgpu,webxr,diagnostics,wry,ux-probes,ux-bridge`
  — production default minus `gl_compat`. **Receipts**: engine-feature
  matrix all 4/4 PASS (default, no-default wry, no-default iced-host
  wry, no-default servo-engine no-gl_compat); 7 render_backend bridge
  tests pass; 38 compositor_adapter tests pass; 40 graphshell-runtime
  tests pass. **Slice 2 (default-off `gl_compat`) remains
  runtime-blocked** — the static gating is honest but the wgpu-only path
  needs end-to-end smoke validation that webview composition succeeds
  without the GL fallback re-registering callbacks; that's a
  runtime-validation receipt, not a static-code one.
- **2026-04-27 (GL legacy survey + Phase B dead-code removal)**:
  surveyed live GL-era surface in graphshell main against the `gl_to_wgpu_plan.md`
  Phase B/F retirement framing, then landed the static-code part of Phase B.
  Findings: `gleam` is not a direct dep (transitive via Servo). `egui_glow`
  is not present (the egui stack is `egui-wgpu` already). `glow = "0.17.0"`
  is unconditional but consumed only by `shell/desktop/render_backend/gl_backend.rs`.
  `surfman` is direct-dep but used only by `shell/desktop/host/accelerated_gl_media.rs`
  (Servo media plumbing, not compositor legacy). The compositor side has
  ~38 `cfg(feature = "gl_compat")` gates plus the content-callback registry
  shape that still threads `BackendGraphicsContext = glow::Context` through.
  Phase B retirement landed: deleted `BackendContentBridge::SharedWgpuTexture`
  variant + `BackendSharedWgpuImport` type alias + `select_content_bridge_wgpu_from_render_context`
  factory — pure dead architectural scaffolding (the actual wgpu shared-texture
  path bypasses `BackendContentBridge` entirely and goes through
  `upsert_native_content_texture` directly). Collapsed
  `BackendContentBridgeSelection` to inline `callback: BackendParentRenderCallback`,
  removed the unreachable `else` branch in
  `register_content_callback_from_render_context`, and re-framed the doc
  comment to explicit "GL parent-render callback used by the GL-compat
  composition path." Receipts: 7 render_backend bridge tests still pass;
  engine-feature matrix all 3/3 PASS. Remaining slices in the GL-retirement
  ordering (NOT landed today): (i) gate `BackendGraphicsContext` /
  `BackendFramebufferHandle` / `BackendParentRenderCallback` and the
  content-callback registry machinery behind `gl_compat`, making `glow`
  optional — needs careful cascade across ~30 `compositor_adapter.rs` use
  sites and runtime validation that the wgpu-only path works without
  unregister/register fallback plumbing; (ii) flip `gl_compat` to off-by-default
  (runtime-validation gated); (iii) Phase F retirement of the 38 GL-state
  guardrails (depends on (ii) being stable). `accelerated_gl_media.rs`
  stays — it's Servo media plumbing, not compositor legacy.
- **2026-04-27 (S3b viewer_surfaces Step 2: RenderingContextProducer trait)**:
  reviewed Servo's `RenderingContextCore` (servo-wgpu/components/shared/paint/rendering_context_core.rs)
  to pick between (a) re-extracting the Servo trait into runtime, (b) opaque
  host-neutral handle, (c) parameterizing over the host context type, and
  (d) deferring entirely. Key findings: Servo's core trait pulls
  `embedder_traits::RefreshDriver`, `webrender_api::units`, `surfman`,
  `gleam`/`glow` — too heavy for graphshell-runtime. Compositor's actual
  consumption from `ViewerSurfaceBacking` is narrow: `size()`, `resize()`,
  `present()`, plus GL-compat `make_current()` / `prepare_for_rendering()`.
  Servo webview construction (`webview_backpressure.rs:328`) consumes the
  full `Rc<dyn RenderingContextCore>` directly, not via the producer trait.
  Decision: minimal `RenderingContextProducer` trait in
  `graphshell-runtime::rendering_context_producer` with primitive-typed
  surface only (no `dpi`, no `webrender_api`, no `surfman`); shell-side
  `ServoRenderingContextProducer` adapter wraps `Rc<dyn RenderingContextCore>`
  and forwards. **Wgpu-first scoping**: trait surface trimmed to
  `size_in_pixels`, `resize`, `present`. GL `make_current` /
  `prepare_for_rendering` were considered but dropped — graphshell is on
  wgpu (Servo lives at `servo-wgpu`; renderer is `webrender-wgpu`), and
  the GL-compat fallback path is gated behind the deprecated `gl_compat`
  feature inside the shell's `OffscreenRenderingContext` consumers. That
  path is path-specific (handled in
  `compositor_adapter::paint_offscreen_content_pass`), not producer-level.
  `ViewerSurfaceBacking` deliberately UNCHANGED — its current Servo
  coupling is fine because `compositor_adapter.rs` is gated on
  `servo-engine` anyway, and Servo webview construction needs the original
  concrete trait. The reshape (changing `NativeRenderingContext` to hold
  `Rc<dyn RenderingContextProducer>`) is a follow-on slice triggered when
  iced-host actually plugs in its own producer; today's slice establishes
  the contract iced will target. Adapter lives at
  `shell/desktop/render_backend/servo_rendering_context_producer.rs`.
  Receipts: graphshell-runtime tests 37 → 40 (3 new trait tests: resize
  observation, present count, object-safety); engine-feature matrix all
  3/3 PASS.
- **2026-04-27 (S3b viewer_surfaces Step 1: handle/frame-path types)**:
  followed the audit's two-step plan for `viewer_surfaces`. Step 1
  extracts the host-neutral lifecycle types: `ContentSurfaceHandle<T>`
  (parameterized over the host's texture-token type, with the pure
  `is_wgpu()` check) and `ViewerSurfaceFramePath` now live in
  `graphshell-runtime::content_surface`. Shell-side
  `compositor_adapter.rs` keeps a `pub(crate) type ContentSurfaceHandle =
  graphshell_runtime::ContentSurfaceHandle<BackendTextureToken>` alias
  plus a free `content_surface_handle_for_node(NodeKey)` function (the
  static `compositor_native_texture_registry()` lookup is shell-owned).
  `ViewerSurfaceFramePath` is now a re-export. The `content_generation:
  u64` counter on `ViewerSurface` is already host-neutral and stays as a
  field — no struct bundling yet (deferred to Step 2 alongside the
  portable `RenderingContextProducer` trait). `ViewerSurfaceBacking`
  (Servo `RenderingContextCore` + `OffscreenRenderingContext`) stays
  shell-side both steps per the audit. Receipts: graphshell-runtime
  tests 35 → 37 (two new content_surface tests for `is_wgpu` and
  frame-path distinctness); engine-feature matrix all 3/3 PASS.
- **2026-04-27 (S3b frame_inbox extraction)**: continued the canonical
  runtime-crate slice path with the next portable-but-shell-owned input
  flagged by the audit. `FrameInboxState` (the typed
  `mpsc::Receiver`-bag plus `drain_flag`/`drain_all` helpers and the four
  `take_*` per-frame consumers) moved into
  `graphshell-runtime::frame_inbox`, with the two drain-coalescing tests
  migrated alongside it. Shell-side `shell/desktop/ui/gui/frame_inbox.rs`
  is now a thin wiring shim: a `pub(crate) type GuiFrameInbox =
  FrameInboxState` alias plus `spawn_gui_frame_inbox(&mut ControlPanel,
  Arc<dyn SignalRouter>) -> GuiFrameInbox` free function that owns the
  ControlPanel-driven subscription wiring (signal types are already
  graphshell-core, so the spawn body stays portable except for the
  `&mut ControlPanel` parameter). The control-panel spawn test stays
  shell-side. Two call sites updated (`gui.rs:419`, `gui_state.rs:769`)
  from `GuiFrameInbox::spawn(...)` to `spawn_gui_frame_inbox(...)`.
  Receipts: graphshell-runtime tests 33 → 35 (two drain tests added);
  shell-side `frame_inbox` test still passes; engine-feature matrix all
  3/3 PASS.
- **2026-04-26 (no-Servo warning cleanup + matrix runner)**: cleaned all
  24 graphshell-lib unused-import warnings under
  `--no-default-features --features iced-host,wry`. Pattern: imports
  consumed only by Servo-gated modules (`render/*`, `host/*`, gated UI
  modules) get `#[allow(unused_imports)]` on the re-export line (matches
  the pre-existing convention in graph_app.rs lines 139/157/195);
  imports consumed only by `cfg(feature = "diagnostics")` or
  `cfg(test)` callers get a parallel `#[cfg(...)]` use line. Files
  touched: `graph_app.rs` (6 re-exports), `app/workbench_layout_policy.rs`,
  `panic_hook.rs`, `shell/desktop/runtime/cli.rs`,
  `shell/desktop/runtime/tracing.rs`,
  `shell/desktop/ui/{command_palette_state, command_surface_telemetry,
  host_ports, omnibar_state, portable_time}.rs`,
  `shell/desktop/workbench/{tile_kind, ux_tree}.rs`, `mods/mod.rs`,
  `registries/atomic/lens/mod.rs`. Receipts: graphshell-lib warnings
  now 0 / 0 / 2 across the no-default-wry / no-default-iced-host,wry /
  default matrix entries (default's two are unchanged egui
  deprecations). Added `scripts/dev/engine-feature-matrix.{sh,ps1}`
  that runs all three combos and emits a PASS/FAIL summary; verified
  end-to-end (3/3 PASS).
- **2026-04-25 (iced-host,wry compile baseline)**: ran
  `cargo check --no-default-features --features iced-host,wry`
  expecting either gated-launch-path residue or runtime-ownership
  errors per the §3 framing. **Result**: clean. 24 warnings (all
  unused imports / one unused macro), 0 errors, 1m 54s cold. The
  iced launch path is still gated on `cfg(all(iced-host,
  servo-engine))` (per S3b.1), so this receipt covers the
  iced-host *bridge surface* (`iced_host_ports`, `CachedTexture`,
  runtime ports) — the library compiles without Servo *and* with
  iced-host's bridge code on. Default `cargo check -p graphshell
  --lib` re-verified clean (2 warnings, pre-existing egui
  deprecations). Implication for sequencing: the next compile-wall
  is no longer the rate-limiter. Whatever S3/S4 means now is about
  making the no-Servo path *launchable*, which routes through the
  canonical GraphshellRuntime extraction (slice-by-slice), not
  more gating.

### S3b proper (in flight): GraphshellRuntime extraction

> **Canonical roadmap:**
> [2026-04-24_graphshell_runtime_crate_plan.md](2026-04-24_graphshell_runtime_crate_plan.md)
> is the authoritative plan for this work. It predates the
> servo-into-verso lane by a day and has already executed Slice 1
> (toast/clipboard ports + finalize helpers + frame-vocabulary
> re-exports) plus the AppState→FrameViewModel projection-helper
> follow-ons for focus/settings/accessibility/graph-search/dialogs/
> toolbar/omnibar/command-palette/transient-outputs. ~18 unit tests
> live inside `graphshell-runtime` against tiny portable inputs.

**Important framing correction.** The earlier draft of this
section recommended "extract `GraphshellRuntime` wholesale" or
"split into `GraphshellRuntimeCore` + extension." Both options
violate the canonical plan's explicit guardrail:

> "Do not move `GraphshellRuntime` wholesale next... Do not hide
> shell ownership behind a giant trait just to move code. If a
> projection helper needs most of `GraphshellRuntime`, it is not
> ready for the runtime crate."

The correct approach is the slice-based incremental extraction
already underway: each slice moves one **portable-but-shell-owned**
input from the canonical plan's inventory (graph_runtime frame
caches, toolbar state/drafts, command-palette state, app settings,
graph-search match collection, dialog objects, thumbnail capture
set) into a runtime-crate-owned type with its own focused unit
tests. The shell side keeps owning the GraphBrowserApp /
audit/diagnostics adapters; only the portable inputs migrate.

The "iced launch path compiles without servo-engine" goal will
come as a natural consequence once enough of the inventory has
moved that `gui_state::GraphshellRuntime`'s remaining fields are
either portable or feature-gated. **Do not try to short-circuit
this with a wholesale extraction.**

**Cross-lane coordination:** S3a's host-port trait extraction
(this plan, 2026-04-25 entry) is additive to the canonical
roadmap — Slice 1 covered toast/clipboard ports; S3a extended
the same `graphshell-runtime::ports` module with the broader
host-port surface (input, surface, paint, texture, accessibility).
Both lanes write to the same crate; neither blocks the other.

## 7. Bottom line

This lane lands the architectural claim that's been implicit in the
recent refactors: **graphshell is a chrome + spatial canvas; the
content engines are pluggable**. Phase A2 proved verso can own a
heavy engine (wry); this lane proves it can own all of them.

Estimated effort: 3–5 sessions of focused work. **Status as of
2026-04-25 end-of-day**: S1, S2a, S2b module-level + body-level
(S2c), S3a host-port extraction, S3b.1 IcedWgpuContext gate, and
the `iced-host,wry` compile baseline all landed today. The
compile-wall portion of the lane is effectively closed: default,
no-default `wry`, and no-default `iced-host,wry` library checks
are all green. **What remains**:

- **S3b proper** — canonical GraphshellRuntime slice-by-slice
  extraction (see
  [2026-04-24_graphshell_runtime_crate_plan.md](2026-04-24_graphshell_runtime_crate_plan.md)).
  This is the path that converts "compiles without Servo" into
  "launches without Servo" by ungating the iced launch path one
  portable input at a time. Do not short-circuit with a wholesale
  extraction.
- **S4** — startup path gating: `cli.rs::main()` no-servo branch
  beyond the current "exit warning" stub once S3b's runtime is
  launchable.
- **S5** — CI matrix doc + cross-doc updates (`PROJECT_DESCRIPTION`,
  `VERSO_AS_PEER`, iced-host migration plan Phase A2 sibling).

Sidequests that would smooth the runway (none blocking): warning
cleanup in no-Servo Wry (24 unused imports), promote duplicated tag
helper logic to a shared home if drift appears, add a documented
compile-matrix command list so the green targets don't regress
silently, narrow tests around no-Servo shims (especially diagnostics
and `workbench_surface::dispatch_intent`), and revisit
`local_file_access`'s home if non-workbench consumers appear.
