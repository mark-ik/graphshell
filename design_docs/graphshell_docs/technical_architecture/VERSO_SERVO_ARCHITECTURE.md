<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Verso Mod & Servo Integration Architecture

**Date**: 2026-02-24  
**Status**: Implementation-Ready (Phase 2.3)  
**Relates to**: 
- [2026-02-22_registry_layer_plan.md](2026-02-22_registry_layer_plan.md) (Mod infrastructure)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (Async mod activation)

---

## Overview

**Servo** = Mozilla's embedded browser engine (Rust, HTML5 layout/rendering)  
**Verso Mod** = Graphshell's native mod for web rendering capabilities  
**Verso's Role** = Register HTTP/HTTPS/data protocols + webview viewer when activated

Verso is **NOT a wrapper around Servo**. Rather, Verso is a **capability provider** that makes Servo's rendering powers accessible through a protocol/viewer contract. The architecture cleanly separates:

1. **Core app** (offline-only graph organizer)
2. **Servo runtime** (embedded browser engine — always compiled in)
3. **Verso mod** (registers Servo capabilities as optional features)

---

## The "Not a Wrapper" Design

**Why Verso isn't a wrapper:**

Traditional architecture might look like:
```
App → Verso (wrapper) → Servo
      ↑
   Tight coupling; Servo features always on
```

**Graphshell architecture instead:**
```
App ──┬─→ ProtocolRegistry ──→ protocol:http (invokes Servo when needed)
      │
      └─→ ViewerRegistry ────→ viewer:webview (invokes Servo when needed)
      
Verso mod registers these contracts (can be disabled)
Servo lives in EmbedderCore (always compiled in, but dormant if Verso inactive)
```

**Key distinction:**
- Servo is **compiled in** (dependency in Cargo.toml)
- Servo is **controlled/accessed** through registry contracts
- Verso **activates** those contracts, but isn't responsible for holding Servo state
- `EmbedderCore` owns Servo instance; Verso just registers how to use it

---

## Handler Registration Flow

### Phase 2.3 Detailed (What's Just Implemented)

**1. Verso exports handler registration functions:**
```rust
pub(crate) fn register_protocol_handlers(providers: &mut ProtocolHandlerProviders) {
    providers.register_fn(|registry| {
        registry.register_scheme("http", "protocol:http");
        registry.register_scheme("https", "protocol:https");
        registry.register_scheme("data", "protocol:data");
    });
}

pub(crate) fn register_viewer_handlers(providers: &mut ViewerHandlerProviders) {
    providers.register_fn(|registry| {
        registry.register_mime("text/html", "viewer:webview");
        registry.register_mime("application/pdf", "viewer:webview");
        registry.register_extension("html", "viewer:webview");
        // ... more MIME types and extensions
    });
}
```

**2. App initialization (pseudo-code, Phase 2.4):**
```rust
fn initialize_registries_from_mods(mod_registry: &ModRegistry) -> Registries {
    let mut protocol_providers = ProtocolHandlerProviders::new();
    let mut viewer_providers = ViewerHandlerProviders::new();
    
    if mod_registry.is_active("verso") {
        verso::register_protocol_handlers(&mut protocol_providers);
        verso::register_viewer_handlers(&mut viewer_providers);
    }
    
    // Verse mod can also register protocol handlers (p2p://, etc.)
    if mod_registry.is_active("verse") {
        verse::register_protocol_handlers(&mut protocol_providers);
    }
    
    // Apply all registered handlers to the registries
    let mut protocol_registry = ProtocolRegistry::core_seed();
    protocol_providers.apply_all(&mut protocol_registry);
    
    let mut viewer_registry = ViewerRegistry::core_seed();
    viewer_providers.apply_all(&mut viewer_registry);
    
    Registries { protocol_registry, viewer_registry, ... }
}
```

---

## Servo Dependency & Update Strategy

### Current Setup (Cargo.toml)
```toml
[dependencies]
servo = { git = "https://github.com/servo/servo.git", branch = "main", ... }
```

**What this means:**
- Servo is fetched from GitHub `main` branch during build
- Every `cargo update` or fresh build pulls latest Servo code
- Servo is fully compiled into the binary (not a runtime plugin)

### Update Procedure

**Rolling updates (recommended for development):**
```bash
# Update all dependencies including Servo
cargo update

# See what changed in Servo
cd target/deps/servo-*
git log --oneline -20  # (if Servo git history is available)

# Test
cargo test --lib
```

**Pinning to a specific Servo commit (for stability):**
```toml
[dependencies]
servo = { git = "https://github.com/servo/servo.git", rev = "abc123def456", ... }
```

Then update by changing the `rev` field:
```bash
# Get latest commit hash from servo repo
git ls-remote https://github.com/servo/servo.git main | head -1

# Update Cargo.toml with new rev, then
cargo update servo
cargo test --lib
```

**Stable release pinning (if Servo releases versions):**
```toml
[dependencies]
servo = { git = "https://github.com/servo/servo.git", tag = "v0.24.0", ... }
```

### Testing Servo Updates

After updating Servo, test:
1. **Unit tests**: `cargo test --lib`
2. **Core seed (offline mode)**: Run graphshell without Verso mod activated
3. **HTTP/HTTPS loading**: Load a homepage with Verso mod active
4. **Crash handling**: Verify webview crashes are handled gracefully
5. **Performance**: Ensure no regressions in frame composition or memory usage

### When Servo Updates Break Things

Servo is an active Mozilla project. Breaking changes may happen:
- Check Servo's [GitHub releases](https://github.com/servo/servo/releases) for breaking changes
- Review the [Servo blog](https://blog.servo.org) for architectural updates
- If breakage occurs, either:
  - **Pin to last known-good commit** and report to Servo
  - **Update Graphshell code** to adapt to Servo API changes
  - **Disable Verso mod** for a build (set `GRAPHSHELL_DISABLE_VERSO=1`) while fixing

### Verso's Relationship to Servo Updates

Verso **decouples Servo from core app**:
- If Servo breaks → Only Verso activation fails, app still runs (offline mode)
- Updates are **non-blocking** for core development
- Can test Servo changes in isolation via Verso mod tests

---

## Protocol/Viewer Resolution

### Without Verso (Core Seed Only)

**Available protocols:**
- `file://` — local files
- `about://` — internal app URLs

**Available viewers:**
- `viewer:plaintext` — text files
- `viewer:metadata` — node properties, tags, history (fallback for unknown MIME types)

**Behavior:**
- User opens node with URL → metadata viewer displays title, favicon, tags, history
- No live rendering, no network access
- Full graph navigation, search, organization

### With Verso (Web Rendering Enabled)

**Added protocols:**
- `http://`, `https://` — web pages
- `data://` — embedded data URIs

**Added viewers:**
- `viewer:webview` — Servo rendering for HTML, PDFs, SVG, CSS, JS
- Registered for MIME types: `text/html`, `application/pdf`, `image/svg+xml`, etc.
- Registered for extensions: `.html`, `.htm`, `.pdf`, `.svg`, etc.

**Behavior:**
- User opens HTTP URL → Servo webview loads and renders the page
- Full DOM interaction, JavaScript, CSS animations
- Form entry, navigation, multimedia playback (if Servo/media supports)

---

## Testing Verso Isolation

**Test offline mode (no Verso):**
```bash
GRAPHSHELL_DISABLE_VERSO=1 cargo run
# Graph organizer works; loading http://example.com shows metadata only
```

**Test with Verso:**
```bash
cargo run
# Same graphshell with http:// loading and rendering
```

**Test Verso registration in unit tests:**
```rust
#[test]
fn verso_protocol_registration() {
    let mut protocol_providers = ProtocolHandlerProviders::new();
    verso::register_protocol_handlers(&mut protocol_providers);
    
    let mut registry = ProtocolContractRegistry::core_seed();
    protocol_providers.apply_all(&mut registry);
    
    assert!(registry.has_scheme("http"));
    assert!(registry.has_scheme("https"));
}
```

---

## Future: Verso Modularity

**Current Architecture (Phase 2.3):**
- Verso is a native mod (compiled in, not dynamically loaded)
- Servo APIs are Rust crates (in-process)

**Future possibilities (Phase 3+):**
- Replace Verso with alternative renderer mods (ChromiumEmbed, CEF, Tauri webview)
- Each mod registers different  protocol/viewer capabilities
- User can build graphshell with preferred rendering engine
- Protocol registry becomes the **renderer abstraction layer**

This is why the protocol/viewer contract design is so important — it makes renderers **swappable**.

---

## Summary

**Verso = Capability registration layer for Servo**
- Not a wrapper or adapter layer
- Registers HTTP/HTTPS/data protocols and webview viewer
- Can be disabled → offline mode
- Servo lives in compiled binary regardless, but only used if Verso activates contracts
- Servo updates are regular (git dependency on main) but non-blocking
- Core app continues working even if Verso registration fails
