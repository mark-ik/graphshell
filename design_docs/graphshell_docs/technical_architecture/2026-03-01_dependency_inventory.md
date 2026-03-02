# Dependency Inventory

**Date**: 2026-03-01
**Status**: Living document — synthesized from `Cargo.toml` and codebase grep audit
**Scope**: All direct Graphshell dependencies (desktop target); build-only and platform-specific deps noted separately
**Sources**: `Cargo.toml`, `Cargo.lock`, codebase grep, `2026-03-01_webrender_wgpu_renderer_research.md`

Status codes:
- **✅ Active** — Imported and in use
- **🔄 Transitional** — Active today; planned to drop or replace (migration noted)
- **📦 Pre-staged** — Declared in `Cargo.toml`; zero `use` statements in codebase; reserved for planned feature work
- **🔧 Build-only** — Used in `build.rs` or platform linker; not imported at runtime
- **🔭 Speculative** — Listed speculatively; no committed implementation schedule

---

## Core Infrastructure

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `servo` (libservo) | ✅ Active | embedder core, webview lifecycle | Git dep from `servo/servo` main; central browser engine |
| `webdriver_server` | ✅ Active | `webdriver.rs` | Git dep from `servo/servo` main; WebDriver protocol server |
| `servo_allocator` | ✅ Active | `main.rs` | System allocator / allocation tracking; git dep |
| `tokio` | ✅ Active | async runtime throughout | `rt`, `rt-multi-thread`, `sync`, `time`, `macros` features |
| `tokio-util` | ✅ Active | `webview_backpressure.rs` | `CancellationToken` for backpressure task shutdown |
| `crossbeam-channel` | ✅ Active | event bus, intent dispatch | MPSC channels for `GraphSemanticEvent` routing |
| `cfg-if` | ✅ Active | platform conditionals | Ubiquitous cross-platform feature gating |
| `log` | ✅ Active | logging throughout | `release_max_level_info` in default features |
| `libc` | ✅ Active | platform calls | Signal handling, process management |
| `backtrace` | ✅ Active | crash diagnostics (non-Android) | Panic handler enrichment |

---

## UI Framework (Desktop)

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `egui` | ✅ Active | all UI panels, panes, overlays | `accesskit` feature; core egui runtime |
| `egui-winit` | ✅ Active | window/event integration | `accesskit`, `clipboard`, `wayland` features |
| `egui_tiles` | ✅ Active | `pane_model.rs`, workbench split/tab tree | `serde` feature; workbench tile layout engine |
| `egui_graphs` | ✅ Active | graph canvas rendering | `events` feature; force-directed graph display |
| `egui_glow` | 🔄 Transitional | `render_backend/mod.rs` | Active GL renderer; drops when wgpu backend lands |
| `egui-notify` | ✅ Active | toast notifications | User-facing transient status messages |
| `egui-file-dialog` | ✅ Active | file open/save dialogs | Profile import/export, resource picker |
| `winit` | ✅ Active | `desktop/` window management | `0.30.12`; event loop, window, input events |
| `arboard` | ✅ Active | clipboard read/write | URL copy, node label copy |
| `accesskit` | ✅ Active | accessibility tree | `serde` feature; egui accessibility bridge |

---

## Rendering

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `glow` | 🔄 Transitional | re-exported via `egui_glow` in `render_backend/mod.rs` | OpenGL abstraction; drops with wgpu migration |
| `surfman` | 🔄 Transitional | `headed_window.rs`, `accelerated_gl_media.rs` | GL surface/context management; drops with wgpu migration |
| `raw-window-handle` | ✅ Active | window handle plumbing | `0.6`; bridge between winit and render backends |
| `dpi` | ✅ Active | HiDPI scale factor | Used throughout event handling and layout |
| `euclid` | ✅ Active | geometry types | Point, Size, Rect throughout |
| `image` | ✅ Active | image decoding | `avif`, `bmp`, `gif`, `ico`, `jpeg`, `png`, `webp`, `rayon` |
| `gilrs` | ✅ Active | gamepad input | `AppGamepadProvider`; gamepad event loop |

---

## Graph Data & Persistence

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `petgraph` | ✅ Active | `pane_model.rs` | Graph topology; `serde-1` feature |
| `serde` | ✅ Active | serialization throughout | `derive` feature; JSON snapshots, config, messages |
| `serde_json` | ✅ Active | JSON snapshots, protocol messages | Graph export, WebDriver wire format |
| `toml` | ✅ Active | `prefs.rs`, config loading | User preferences file format |
| `uuid` | ✅ Active | node/edge/session IDs | `serde`, `v4` features |
| `rkyv` | 📦 Pre-staged | — | Zero-copy serialization; intended for high-throughput graph persistence path |
| `fjall` | 📦 Pre-staged | — | LSM-tree embedded KV store; planned for Verse sync log and graph WAL v2 |
| `redb` | 📦 Pre-staged | — | Embedded ACID KV; evaluated alongside `fjall` for graph/session persistence |
| `zstd` | 📦 Pre-staged | — | Compression; planned for snapshot export and network sync payloads |

---

## Networking & P2P (Verse)

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `iroh` | ✅ Active | `verse/` P2P transport | QUIC-based peer transport; Verse graph sync |
| `ed25519-dalek` | ✅ Active | `verse/` key management | `serde` feature; used directly via iroh key generation path |
| `rand` | ✅ Active | `verse/` | Random key material for iroh; no other direct uses found |
| `reqwest` | ✅ Active | HTTP resource fetching | `blocking`, `rustls-tls` features; resource node fetch |
| `rustls` | ✅ Active | TLS for reqwest | `aws-lc-rs` backend; `tls12`, `std` |
| `http` | ✅ Active | HTTP types | Header/status type integration |
| `headers` | ✅ Active | typed HTTP headers | Typed `Content-Type`, `Authorization` etc. |
| `url` | ✅ Active | URL parsing and normalization | Node URLs, navigation targets |
| `mime_guess` | ✅ Active | `resource.rs`, `protocol.rs` | MIME detection for viewer selection |
| `infer` | 📦 Pre-staged | — | Binary MIME sniffing from file bytes; planned complement to `mime_guess` for local file nodes |
| `tower` | 📦 Pre-staged | — | Service/middleware abstractions; planned for internal HTTP-like request routing |
| `mdns-sd` | 📦 Pre-staged | — | mDNS discovery; planned for Verse local peer discovery (LAN pairing) |
| `hostname` | ✅ Active | `verse/` | Local hostname for peer advertisement |
| `backon` | ✅ Active | `webview_backpressure.rs` | Exponential retry policy for webview task restart |

---

## Cryptography & Security

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `aes-gcm` | 📦 Pre-staged | — | AES-256-GCM encryption; planned for graph persistence at-rest encryption (feature inventory says "done" — verify against `fjall` path) |
| `sha2` | 📦 Pre-staged | — | SHA-256/512; planned for content-addressable node hashing and deduplication |
| `keyring` | 📦 Pre-staged | — | OS keychain integration; planned for Verse identity key storage |
| `base64` | 📦 Pre-staged | — | Base64 encode/decode; planned for key export and QR code payload encoding |
| `qrcode` | 📦 Pre-staged | — | QR code rendering; planned for Verse device pairing flow |

---

## Search & Indexing

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `nucleo` | 📦 Pre-staged | — | Fuzzy matching engine; planned for Command Palette and node search |
| `rstar` | 📦 Pre-staged | — | R-tree spatial index; planned for lasso-select region queries on large graphs |

---

## Diagnostics & Tracing

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `sysinfo` | ✅ Active | `lifecycle_reconcile.rs`, `control_panel.rs` | Memory/CPU pressure for lifecycle eviction |
| `tracing` | ✅ Active (optional) | `tracing` feature | Structured spans; disabled in default build |
| `tracing-subscriber` | ✅ Active (optional) | `tracing` feature | `env-filter` for runtime log level |
| `tracing-perfetto` | 🔭 Speculative (optional) | `tracing-perfetto` feature | Perfetto trace export; no production use yet |
| `hitrace` | 🔭 Speculative (optional) | `tracing-hitrace` feature | HarmonyOS tracing; OHOS target only |
| `inventory` | 📦 Pre-staged | — | Linkage-time plugin registration; planned for `DiagnosticChannelDescriptor` and `ActionRegistry` static registration |

---

## CLI & Config

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `bpaf` | ✅ Active | `main.rs` CLI parsing | `derive` feature; command-line flags |
| `dirs` | ✅ Active | `prefs.rs` | XDG/platform data dirs for profile paths |

---

## Keyboard & Input

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `keyboard-types` | ✅ Active | input event handling | `serde`, `webdriver` features; key code normalization |

---

## Spatial Browser Extras

| Dep | Status | Used in | Notes |
|-----|--------|---------|-------|
| `iroh` | ✅ Active | see Networking | (listed once; belongs to Verse) |

---

## Build-only Dependencies

| Dep | Status | Target | Notes |
|-----|--------|--------|-------|
| `cc` | 🔧 Build-only | macOS | C/C++ compiler for macOS native glue in `build.rs` |
| `winresource` | 🔧 Build-only | Windows | Embeds version/icon resources into `.exe` |

---

## Platform-specific Runtime Dependencies

| Dep | Status | Target | Notes |
|-----|--------|--------|-------|
| `windows-sys` | ✅ Active | Windows | `Win32_Graphics_Gdi`, `Win32_System_Console`; console mode and window management |
| `sig` | ✅ Active | Linux, macOS | POSIX signal handler |
| `objc2-app-kit` | ✅ Active | macOS | NSView/NSWindow integration |
| `objc2-foundation` | ✅ Active | macOS | NSObject, NSString bridge |
| `android_logger` | ✅ Active | Android | logcat log backend |
| `jni` | ✅ Active | Android | JNI bridge for Android embedding |
| `nix` | ✅ Active | Android, OHOS | Unix fs/process APIs |
| `surfman` (ANGLE) | 🔄 Transitional | Android, OHOS | ANGLE-based GL surface; separate entry from desktop `surfman` |
| `base` (servo) | ✅ Active | Android, OHOS | Servo base utilities; git dep |

---

## Summary

| Category | Count | Notes |
|----------|-------|-------|
| ✅ Active | ~40 | In use, no plans to drop |
| 🔄 Transitional | 3 | `glow`, `egui_glow`, `surfman` — drop with wgpu migration |
| 📦 Pre-staged | 15 | Zero usage today; reserved for future features |
| 🔧 Build-only | 2 | `cc`, `winresource` |
| 🔭 Speculative | 2 | `tracing-perfetto`, `hitrace` (optional features, no production use) |

### Pre-staged group — planned feature association

| Dep | Planned feature |
|-----|----------------|
| `fjall` | Graph WAL v2 / Verse sync log (LSM-tree) |
| `redb` | Evaluated alternative to `fjall` for graph/session persistence |
| `rkyv` | Zero-copy serialization for high-throughput persistence path |
| `zstd` | Snapshot compression, network sync payloads |
| `nucleo` | Command Palette fuzzy search, node title search |
| `rstar` | Lasso-select R-tree spatial queries on large graphs |
| `infer` | Binary MIME sniffing for local file node viewer selection |
| `tower` | Internal HTTP-like request routing middleware |
| `mdns-sd` | Verse LAN peer discovery (mDNS) |
| `qrcode` | Verse device pairing QR code display |
| `base64` | Key export, QR code payload encoding |
| `aes-gcm` | Graph at-rest encryption (cross-check: feature inventory says done — verify) |
| `sha2` | Content-addressable node hash / deduplication |
| `keyring` | OS keychain storage for Verse Ed25519 identity key |
| `inventory` | Static registration for `DiagnosticChannelDescriptor`, `ActionRegistry` entries |

### Transitional group — wgpu migration path

All three transitional deps (`glow`, `egui_glow`, `surfman`) are dropped together when the wgpu renderer backend lands. See `2026-03-01_webrender_wgpu_renderer_research.md` for the migration plan. The `egui` team ships a `wgpu`-backed renderer (`egui_wgpu`) as a drop-in for `egui_glow` — no egui API changes required at the call sites.
