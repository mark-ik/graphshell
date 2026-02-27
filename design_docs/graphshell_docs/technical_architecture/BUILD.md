# Building Graphshell

This guide covers building Graphshell on Windows, macOS, and Linux.

Graphshell is a standalone Rust crate. Servo is pulled as a git dependency — no local Servo checkout needed, no `mach` required.

> Cargo is the canonical default build path. If a command can be done with cargo directly, prefer that first. The `scripts/dev/*` helpers are optional wrappers for convenience/lane management.

---

## Quick Start (Standalone)

```bash
# Build
cargo build

# Run (debug)
cargo run -- https://example.com

# Run (release)
cargo run --release -- https://example.com

# Run with logging
RUST_LOG=debug cargo run -- https://example.com

# Headless
cargo run --release -- --headless https://example.com
```

Controls and shortcuts live in [KEYBINDINGS.md](../design/KEYBINDINGS.md).

---

## Cargo-first defaults (what "default build" means)

By default, cargo uses the **debug/dev profile** unless `--release` is specified.

- `cargo build` compiles with faster build times and debug symbols.
- `cargo run` runs the debug build.
- `cargo test` builds and runs tests in debug profile by default.
- `cargo check` type-checks and validates without full codegen.

For most contributors, this is the right day-to-day loop:

```bash
cargo check
cargo test
cargo run -- https://example.com
```

Use release profile when validating runtime behavior/perf characteristics:

```bash
cargo build --release
cargo run --release -- https://example.com
```

### Managed target directories (cross-platform)

These scripts are optional. Prefer direct cargo commands first; use scripts when you want lane-safe target directory routing and convenience wrappers.

You can audit/install a recommended CLI baseline with:

```bash
scripts/dev/bootstrap-dev-env.sh
scripts/dev/bootstrap-dev-env.sh --install
scripts/dev/bootstrap-dev-env.sh --install-pwsh
```

```powershell
pwsh -File scripts/dev/bootstrap-dev-env.ps1
pwsh -File scripts/dev/bootstrap-dev-env.ps1 --install
pwsh -File scripts/dev/bootstrap-dev-env.ps1 --install-pwsh
```

To avoid mixing build artifacts between host environments, use the helper scripts in `scripts/dev/`:

```bash
# Bash (WSL/Linux/macOS)
scripts/dev/smoke-matrix.sh status
scripts/dev/smoke-matrix.sh quick
scripts/dev/smoke-matrix.sh cargo build --release
```

```powershell
# PowerShell (Windows/local VS Code)
pwsh -File scripts/dev/smoke-matrix.ps1 status
pwsh -File scripts/dev/smoke-matrix.ps1 quick
pwsh -File scripts/dev/smoke-matrix.ps1 cargo build --release
```

Default output directories are selected by host lane and created on demand:

- Linux/WSL: `target/linux_target`
- Windows: `target/windows_target`
- macOS: `target/macos_target`

Optional lane controls:

- `GRAPHSHELL_CARGO_LANE=linux|windows|macos` force lane selection
- `GRAPHSHELL_LINUX_TARGET_FLAVOR=<name>` split Linux outputs when needed (for example `linux_target-ubuntu`)
- `GRAPHSHELL_SPLIT_WSL_TARGET=1` split WSL to `linux_target-wsl`
- `CARGO_TARGET_DIR=<path>` full manual override

---

## Prerequisites

### Rust Toolchain

The toolchain is pinned in `rust-toolchain.toml` (currently 1.92.0). Cargo will install it automatically on first use.

```bash
# Verify
rustc --version
cargo --version
```

### System Dependencies

Servo requires native libraries for its rendering pipeline. Install them for your platform:

#### Windows

- Visual Studio 2022 Build Tools (MSVC toolchain)
- [LLVM/Clang](https://releases.llvm.org/) (required by bindgen)
- CMake

#### macOS

```bash
xcode-select --install
brew install cmake pkg-config
```

#### Linux (Debian/Ubuntu)

```bash
sudo apt-get install -y \
  cmake pkg-config libssl-dev \
  libglib2.0-dev libgtk-3-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libfontconfig1-dev \
  clang llvm-dev
```

#### Linux (Fedora)

```bash
sudo dnf install cmake clang pkg-config openssl-devel \
  gtk3-devel fontconfig-devel
```

---

## Build

```bash
# Debug build (faster compile, slower runtime)
cargo build

# Release build (recommended for use)
cargo build --release
```

First build fetches and compiles Servo from `github.com/servo/servo.git` — expect 15–30 minutes depending on hardware. Incremental builds after that are 30s–2min.

---

## Run

```bash
# Debug
cargo run -- https://example.com

# Release (recommended)
cargo run --release -- https://example.com

# With logging
RUST_LOG=debug cargo run -- https://example.com

# Headless mode
cargo run --release -- --headless https://example.com
```

---

## Test

```bash
# All tests
cargo test

# Specific test
cargo test test_name --lib

# With output
cargo test test_name --lib -- --nocapture

# Single integration test target
cargo test --test <integration_test_name>

# Single package target (workspace-safe habit)
cargo test -p graphshell

# Deterministic single-threaded test run (for flaky triage)
cargo test -- --test-threads=1

# Keep warnings/errors visible in debug profile checks
cargo check
```

### Debug-profile testing workflow (recommended default)

1. `cargo check`
2. `cargo test` (or targeted test first)
3. `cargo run -- <url>` for local behavior checks

This sequence matches how most contributors iterate quickly while preserving debuggability.

---

## Lint & Format

```bash
cargo fmt          # Format code
cargo clippy       # Lint
cargo check        # Fast type-check without codegen
cargo doc --no-deps # Build local API docs for this crate
```

---

## Clean

```bash
cargo clean        # Remove build artifacts (next build will be a full rebuild)
```

---

## Build Notes

### `build.rs`

The build script (`build.rs`) runs automatically with cargo and handles:
- Capturing git SHA for the version string
- **Windows:** bundling the app icon and manifest via `winresource`
- **macOS:** compiling a small C helper for thread counting
- **Android:** NDK 23c+ libgcc→libunwind workaround

No manual steps needed — these all work with plain `cargo build`.

### Servo Git Dependency

`Cargo.toml` pins Servo to `github.com/servo/servo.git` at the `main` branch. `Cargo.lock` captures the exact commit used. To update Servo:

```bash
cargo update -p servo
```

### Feature Flags

Default features: `gamepad`, `servo/clipboard`, `js_jit`, `max_log_level`, `webgpu`, `webxr`

Optional: `debugmozjs`, `media-gstreamer`, `native-bluetooth`, `tracing`, `vello`

```bash
# Example: build with GStreamer media support
cargo build --release --features media-gstreamer
```

---

## Platform Notes

| Platform | Status | Notes |
| --- | --- | --- |
| Windows | ✅ | MSVC toolchain, WGL disabled for WebGL |
| macOS | ✅ | Xcode CLT required |
| Linux | ✅ | Wayland + X11 via egui-winit |
| Android | Library only | Compile as `cdylib`, integrate with APK |
| OpenHarmony | Library only | NAPI/AbiKit integration |

