# Building Graphshell

This guide covers building Graphshell on Windows, macOS, and Linux.

Graphshell is a standalone Rust crate. Servo is pulled as a git dependency — no local Servo checkout needed, no `mach` required.

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
```

---

## Lint & Format

```bash
cargo fmt          # Format code
cargo clippy       # Lint
cargo check        # Fast type-check without codegen
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

