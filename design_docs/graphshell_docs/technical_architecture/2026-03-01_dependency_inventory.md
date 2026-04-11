# Dependency Inventory

**Date**: 2026-03-01
**Status**: Living document — synthesized from `Cargo.toml`, codebase grep audit, and doc scan
**Scope**: All direct Graphshell dependencies (desktop target); build-only and platform-specific deps noted separately; crates of interest tracked for future adoption
**Sources**: `Cargo.toml`, `Cargo.lock`, codebase grep, all `design_docs/graphshell_docs/` strategy docs

Status codes — **Current dependencies**:

- **✅ Active** — Imported and in use
- **🔄 Transitional** — Active today; planned to drop or replace (migration noted)
- **📦 Pre-staged** — Declared in `Cargo.toml`; zero `use` statements; reserved for planned feature work
- **🔧 Build-only** — Used in `build.rs` or platform linker; not imported at runtime
- **🔭 Speculative** — Declared but no committed implementation schedule

Status codes — **Crates of interest** (not yet in `Cargo.toml`):

- **🟢 Adopt when ready** — Maintained, license-compatible, directly maps to a planned feature; clear adoption trigger
- **🟡 Watch** — Good fit but requires validation (version conflict, license check, maturity concern, or competing option still open)
- **🔴 Ruled out** — Evaluated and rejected; reason documented to avoid re-evaluation

Maintenance convention: "Active" = released 2025 or 2026. "Stable" = no recent releases but API is frozen by design.

---

## Part 1 — Current Dependencies

### Core Infrastructure

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
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

### UI Framework (Desktop)

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `egui` | ✅ Active | all UI panels, panes, overlays | `accesskit` feature; core egui runtime |
| `egui-winit` | ✅ Active | window/event integration | `accesskit`, `clipboard`, `wayland` features |
| `egui_tiles` | ✅ Active | `pane_model.rs`, workbench split/tab tree | `serde` feature; workbench tile layout engine |
| `egui_graphs` | ✅ Active | graph canvas rendering | `events` feature; force-directed graph display |
| `egui-wgpu` | ✅ Active | `render_backend/mod.rs`, `render_backend/wgpu_backend.rs` | Current egui renderer backend |
| `egui-notify` | ✅ Active | toast notifications | User-facing transient status messages |
| `egui-file-dialog` | ✅ Active | file open/save dialogs | Profile import/export, resource picker |
| `winit` | ✅ Active | `desktop/` window management | `0.30.12`; event loop, window, input events |
| `arboard` | ✅ Active | clipboard read/write | URL copy, node label copy |
| `accesskit` | ✅ Active | accessibility tree | `serde` feature; egui accessibility bridge |

---

### Rendering

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `glow` | 🔄 Transitional | `render_backend/gl_backend.rs` | OpenGL abstraction retained for the Servo parent-render callback bridge |
| `surfman` | 🔄 Transitional | `headed_window.rs`, `accelerated_gl_media.rs` | GL surface/context management; drops with wgpu migration |
| `raw-window-handle` | ✅ Active | window handle plumbing | `0.6`; bridge between winit and render backends |
| `dpi` | ✅ Active | HiDPI scale factor | Used throughout event handling and layout |
| `euclid` | ✅ Active | geometry types | Point, Size, Rect throughout |
| `image` | ✅ Active | image decoding | `avif`, `bmp`, `gif`, `ico`, `jpeg`, `png`, `webp`, `rayon` |
| `gilrs` | ✅ Active | gamepad input | `AppGamepadProvider`; gamepad event loop |

---

### Graph Data & Persistence

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
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

### Networking & P2P

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
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

### Cryptography & Security

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `aes-gcm` | 📦 Pre-staged | — | AES-256-GCM encryption; no `use` statements found — WAL encryption is planned for the `fjall`-backed SyncLog (feature inventory "done" status refers to the architectural decision, not the implementation) |
| `sha2` | 📦 Pre-staged | — | SHA-256/512; planned for content-addressable node hashing and deduplication |
| `keyring` | 📦 Pre-staged | — | OS keychain integration; planned for Verse Ed25519 identity key storage |
| `base64` | 📦 Pre-staged | — | Base64 encode/decode; planned for key export and QR code payload encoding |
| `qrcode` | 📦 Pre-staged | — | QR code rendering; planned for Verse device pairing flow |

---

### Search & Indexing

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `nucleo` | 📦 Pre-staged | — | Fuzzy matching engine; planned for Command Palette and node search |
| `rstar` | 📦 Pre-staged | — | R-tree spatial index; planned for lasso-select region queries on large graphs |

---

### Diagnostics & Tracing

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `sysinfo` | ✅ Active | `lifecycle_reconcile.rs`, `control_panel.rs` | Memory/CPU pressure for lifecycle eviction |
| `tracing` | ✅ Active (optional) | `tracing` feature | Structured spans; disabled in default build |
| `tracing-subscriber` | ✅ Active (optional) | `tracing` feature | `env-filter` for runtime log level |
| `tracing-perfetto` | 🔭 Speculative (optional) | `tracing-perfetto` feature | Perfetto trace export; no production use yet |
| `hitrace` | 🔭 Speculative (optional) | `tracing-hitrace` feature | HarmonyOS tracing; OHOS target only |
| `inventory` | 📦 Pre-staged | — | Linkage-time plugin registration; planned for `DiagnosticChannelDescriptor` and `ActionRegistry` static registration |

---

### CLI & Config

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `bpaf` | ✅ Active | `main.rs` CLI parsing | `derive` feature; command-line flags |
| `dirs` | ✅ Active | `prefs.rs` | XDG/platform data dirs for profile paths |

---

### Keyboard & Input

| Dep | Status | Used in | Notes |
| --- | --- | --- | --- |
| `keyboard-types` | ✅ Active | input event handling | `serde`, `webdriver` features; key code normalization |

---

### Build-only Dependencies

| Dep | Status | Target | Notes |
| --- | --- | --- | --- |
| `cc` | 🔧 Build-only | macOS | C/C++ compiler for macOS native glue in `build.rs` |
| `winresource` | 🔧 Build-only | Windows | Embeds version/icon resources into `.exe` |

---

### Platform-specific Runtime Dependencies

| Dep | Status | Target | Notes |
| --- | --- | --- | --- |
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

### Summary — Current Dependencies

| Category | Count | Notes |
| --- | --- | --- |
| ✅ Active | ~40 | In use, no plans to drop |
| 🔄 Transitional | 2 | `glow`, `surfman` — retained for the Servo parent-render callback bridge |
| 📦 Pre-staged | 15 | Zero usage today; reserved for future features |
| 🔧 Build-only | 2 | `cc`, `winresource` |
| 🔭 Speculative | 2 | `tracing-perfetto`, `hitrace` (optional features, no production path yet) |

#### Pre-staged → planned feature map

| Dep | Planned feature |
| --- | --- |
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
| `aes-gcm` | SyncLog at-rest AES-256-GCM encryption (Verse Tier 1 prerequisite) |
| `sha2` | Content-addressable node hash / deduplication |
| `keyring` | OS keychain storage for Verse Ed25519 identity key |
| `inventory` | Static registration for `DiagnosticChannelDescriptor`, `ActionRegistry` entries |

#### Transitional group — bridge retirement path

`egui-wgpu` is now the active Graphshell UI backend. `glow` and `surfman` remain only because Servo content still enters the compositor through a GL parent-render callback bridge. See `2026-03-01_webrender_wgpu_renderer_research.md` for the remaining bridge-retirement plan.

---

## Part 2 — Crates of Interest

Crates tracked for future adoption, plus one chronology marker for a render-path crate that has since been adopted. Verified against crates.io as of 2026-03-01 unless noted otherwise.

Rules applied:

- Maintained only (last release 2024 or later, unless API-stable by design)
- No duplicate functionality (one winner per domain unless the runners-up are explicitly documented)
- License must be compatible: MIT, Apache-2.0, BSD-2/3, MPL-2.0, CC0, or LLVM-exception variants

---

### Rendering — wgpu Migration

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `egui-wgpu` | 0.34.1 | MIT OR Apache-2.0 | ✅ Adopted | Current egui renderer backend in Graphshell |
| `egui_extras` | 0.33.3 | MIT OR Apache-2.0 | 🟢 Adopt when ready | `TableBuilder` for `viewer:csv`; `RetainedImage` for image viewer; same release cadence as egui |
| `tiny-skia` | 0.12.0 | BSD-3-Clause | 🟢 Adopt when ready | Pulled transitively by `resvg`; may need direct dep for pixel-level canvas ops. BSD-3 compatible |
| `wgpu` | 26.0.1 | MIT OR Apache-2.0 | 🟢 Adopt when ready | Already in lock file via servo. Direct dep needed for wgpu canvas migration |

---

### Viewer System — Non-Web Content

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `resvg` | 0.47.0 | Apache-2.0 OR MIT | 🟢 Adopt when ready | Pure-Rust SVG rasterizer (linebender); renders to `tiny_skia::Pixmap`. Adoption trigger: `viewer:image` milestone |
| `pulldown-cmark` | 0.13.1 | MIT | 🟢 Adopt when ready | CommonMark Markdown parser; zero deps, fast. Pair with `egui_commonmark` for egui rendering |
| `egui_commonmark` | 0.22.0 | MIT OR Apache-2.0 | 🟢 Adopt when ready | Renders `pulldown-cmark` AST to egui. Adoption trigger: `viewer:plaintext` Markdown mode |
| `syntect` | 5.3.0 | MIT | 🟢 Adopt when ready | TextMate grammar syntax highlighting; 400+ languages. For `viewer:plaintext` code files |
| `pdfium-render` | 0.8.37 | MIT OR Apache-2.0 | 🟡 Watch | PDFium C FFI; requires bundled `.dll`/`.so`. License OK. **Concern**: native binary distribution. Feature-gate as `pdf`. MIT license; check PDFium itself (BSD-3) |
| `symphonia` | 0.5.5 | MPL-2.0 | 🟡 Watch | Pure-Rust audio decoder (MP3/FLAC/OGG/WAV/AAC). **License**: MPL-2.0 — file-level copyleft, compatible with closed distribution if files not modified. Feature-gate as `audio` |
| `rodio` | 0.22.1 | MIT OR Apache-2.0 | 🟢 Adopt when ready | Audio playback; wraps `cpal` for OS audio (WASAPI on Windows). Pair with `symphonia`. Feature-gate as `audio` |
| `cpal` | 0.17.3 | Apache-2.0 | 🟡 Watch | OS audio I/O pulled transitively by `rodio`; may need direct dep for audio device control. Adoption: same trigger as `rodio` |
| `wry` | 0.54.2 | Apache-2.0 OR MIT | 🟡 Watch | Native OS webview (WKWebView/WebView2/WebKitGTK). Planned for `viewer:wry` NativeOverlay mode. **Concern**: shares winit event loop; Linux requires GTK event loop. Windows-first adoption per `2026-02-23_wry_integration_strategy.md` |

---

### Search & Knowledge

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `tantivy` | 0.25.0 | MIT | 🟢 Adopt when ready | Pure-Rust full-text search engine. Planned for local node content index and Verse `IndexArtifact` segments. Adoption trigger: full-text search milestone |
| `scraper` | 0.25.0 | ISC | 🟡 Watch | HTML scraping / DOM selection (CSS selectors). ISC license (permissive). Useful for readability extraction and clipping-feature DOM traversal. No pure-Rust WARC/readability alternative; watch `dom_query` as a lighter option |
| `usearch` | 2.24.0 | Apache-2.0 | 🟡 Watch | HNSW approximate nearest-neighbor vector search (Unum). Used for semantic node similarity. **Alternative** to building on `tantivy` vector fields. Watch until embedding pipeline (agent tier) is clearer |

---

### Sonification & Audio

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `fundsp` | 0.23.0 | MIT OR Apache-2.0 | 🟡 Watch | Procedural audio synthesis DSP graph. Planned for accessibility sonification (density hum, spatial cues). Speculative feature; watch until accessibility sonification milestone approaches |

---

### Networking & P2P (Verse Tier 2)

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `libp2p` | 0.56.0 | MIT | 🟡 Watch | Full P2P networking stack (GossipSub, DHT, Identify, Relay). Planned for Verse public community network (Tier 2, Q3 2026 research validation). Watch until Verse Tier 2 research begins |
| `iroh-blobs` | 0.98.0 | MIT OR Apache-2.0 | 🟡 Watch | Content-addressed blob transfer (BLAKE3 CIDs, Bitswap-style). Same iroh ecosystem as current `iroh` dep. Planned for `VerseBlob` content addressing. Adoption trigger: Verse Tier 1 content sync milestone |
| `nostr-sdk` | 0.44.1 | MIT | 🟡 Watch | Rust Nostr protocol SDK. Optional signaling layer for Verse bootstrap peer discovery. Speculative; watch until Verse Tier 2 architecture is committed |

---

### Cryptography (Verse Content Addressing)

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `blake3` | 1.8.3 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | 🟢 Adopt when ready | BLAKE3 cryptographic hash; the canonical CID function for `VerseBlob` and content-addressable node deduplication. Extremely fast; pure-Rust. Adoption trigger: Verse content addressing or dedup milestone |

---

### WASM Mod Runtime

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `extism` | 1.13.0 | BSD-3-Clause | 🟢 Adopt when ready | WASM plugin runtime wrapping Wasmtime; capability-restricted host interface. Canonical choice per `SUBSYSTEM_MODS.md` and registry layer plan. BSD-3 compatible. Adoption trigger: WASM mod milestone |
| `wasmtime` | 42.0.1 | Apache-2.0 WITH LLVM-exception | 🟡 Watch | Lower-level WASM runtime pulled transitively by `extism`. Direct dep only if custom WASM host interface is needed beyond `extism` ABI. LLVM-exception compatible |
| `cranelift-jit` | 0.129.1 | Apache-2.0 WITH LLVM-exception | 🟡 Watch | JIT code generation backend (BytecodeAlliance). Relevant if Nova JS engine research proceeds (script engine alternatives research doc). Not needed for current mod plan |

---

### AI / ML Inference (Agent Tier)

| Crate | Latest | License | Status | Notes |
| --- | --- | --- | --- | --- |
| `candle-core` | 0.9.2 | MIT OR Apache-2.0 | 🟡 Watch | HuggingFace pure-Rust ML framework. CPU + CUDA + Metal backends. Planned for local tiny-model agent tier (STM/LTM, UDC auto-tagging). No ONNX support; weights must be in safetensors/GGUF. Watch until agent tier milestone |
| `ort` | 2.0.0-rc.11 | MIT OR Apache-2.0 | 🟡 Watch | ONNX Runtime Rust bindings (pykeio). Broader model compatibility than `candle` (any ONNX-exported model). Release candidate; wait for stable 2.0. **Alternative to `candle`** — pick one for the agent tier |
| `fastembed` | 5.11.0 | Apache-2.0 | 🟡 Watch | Fast text/image embeddings (wraps `ort`). Useful for semantic node similarity and UDC tag suggestion without full LLM. If `ort` is adopted, `fastembed` follows naturally |

---

### Ruled Out

| Crate | Reason |
| --- | --- |
| `mupdf-sys` | AGPL-3.0; requires licensing the entire app or Artifex commercial license. Use `pdfium-render` instead |
| `readability` (crate) | Last release 2023; abandoned. Use `scraper` for DOM extraction instead |
| `holyjit` | Project abandoned; last activity 2019. Use `cranelift-jit` for JIT needs |
| `llvm-sys` | LLVM GPL-with-exception; too heavy and license-complex for JIT use. `cranelift-jit` is the correct choice |
| `ohim` | WIT-based DOM binding layer for Wasm component model; mutually exclusive with Nova-based script engine track. Not compatible with Servo's existing JS execution model |

---

## Part 3 — Adoption Decision Log

Tracks decisions made when a crate moves from "of interest" to adopted or ruled out. Update this log when `Cargo.toml` changes.

| Date | Crate | Decision | Rationale |
| --- | --- | --- | --- |
| 2026-03-01 | `readability` | Ruled out | Abandoned 2023; `scraper` is the maintained alternative |
| 2026-03-01 | `mupdf-sys` | Ruled out | AGPL incompatible; `pdfium-render` chosen |
| 2026-03-01 | `holyjit` | Ruled out | Abandoned project |
| 2026-03-01 | `ohim` | Ruled out | Architecture mismatch with Servo JS model; tracked in `2026-03-01_servo_script_engine_alternatives.md` |
| 2026-03-01 | `candle-core` vs `ort` | Watch (open) | Both valid; pick one when agent tier milestone begins. `candle` = pure Rust; `ort` = broader model compatibility |
