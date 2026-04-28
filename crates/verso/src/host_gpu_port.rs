/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host GPU context contract for engine-to-host texture sharing.
//!
//! `HostGpuPort` is the contract the host graphics context (iced) implements,
//! giving content engines (Servo, wry frame capture) a typed entry point for:
//!
//! 1. Importing engine-produced textures into the host's wgpu device.
//! 2. Coordinating command-encoder submissions so compositor UI passes and
//!    content composition share a single frame's submit ordering.
//! 3. Answering capabilities queries.
//!
//! Associated types keep the trait portable — no direct wgpu dependency in
//! verso. The concrete iced host impl supplies `Arc<wgpu::Device>` etc.
//!
//! Resolves the "Servo shared device" ambiguity from §3/§4 of
//! `2026-04-25_servo_into_verso_plan.md`: iced provides the host-owned GPU
//! context; Servo-produced textures are one import source via
//! `import_content_texture` rather than the device owner.

/// Capabilities the host GPU context exposes to content engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostGpuCapabilities {
    /// Host can import texture handles from a content engine that shares the
    /// same wgpu adapter. When `true`, `HostGpuPort::import_content_texture`
    /// will succeed for same-adapter producers (e.g., Servo sharing iced's
    /// wgpu device).
    pub supports_shared_texture_import: bool,
    /// Host supports encoder-coordinated command submissions — content
    /// composition and compositor UI passes can share a single frame's submit
    /// ordering via `HostGpuPort::with_encoder`.
    pub supports_encoder_coordination: bool,
}

/// Contract the host graphics context (iced) implements to:
///
/// - Expose its wgpu device/queue as typed opaque handles so content engines
///   can request resource allocation.
/// - Accept content-engine-produced textures (Servo's wgpu output, wry frame
///   captures) and mint stable tokens that iced's render loop can reference.
/// - Coordinate command-encoder submissions so compositor UI passes and content
///   composition share a single frame's submit ordering.
///
/// # Associated types
///
/// All GPU-specific types are associated so verso carries no wgpu dependency.
/// The concrete iced impl supplies:
/// - `DeviceHandle = Arc<wgpu::Device>`
/// - `QueueHandle  = Arc<wgpu::Queue>`
/// - `TextureToken = wgpu::TextureId` (or equivalent iced handle)
/// - `Encoder      = wgpu::CommandEncoder`
/// - `ContentTextureSource` = the wgpu texture view / descriptor produced by
///   the content engine (e.g., webrender-wgpu's output surface)
///
/// # Object safety
///
/// This trait is intentionally not object-safe — `with_encoder` takes a
/// generic `FnOnce` to avoid the `Box<dyn FnOnce>` indirection on the hot
/// frame path. It is always bound statically from whichever host type the
/// compositor adapter is parameterised over.
pub trait HostGpuPort {
    /// Opaque device handle. Iced impl: `Arc<wgpu::Device>`.
    type DeviceHandle: Clone;
    /// Opaque queue handle. Iced impl: `Arc<wgpu::Queue>`.
    type QueueHandle: Clone;
    /// Stable token for a content texture registered in the host's wgpu
    /// device. Stored as `ContentSurfaceHandle::ImportedWgpu(token)` in
    /// `graphshell-runtime`.
    type TextureToken: Clone;
    /// Command encoder type for GPU command recording.
    /// Iced impl: `wgpu::CommandEncoder`.
    type Encoder;
    /// Descriptor or view passed in by the content engine when handing off a
    /// produced texture. Iced+Servo impl: the wgpu `TextureView` (or
    /// equivalent) from webrender-wgpu's output surface.
    type ContentTextureSource;

    /// Query what this GPU context supports for content integration.
    fn gpu_capabilities(&self) -> HostGpuCapabilities;

    /// Import a content-engine-produced texture into the host's wgpu device.
    ///
    /// `surface_id_packed` is the packed viewer-surface identity —
    /// `ViewerSurfaceId::as_u64()` from `graphshell-runtime`. Returns a stable
    /// `TextureToken` if import succeeds, `None` otherwise (mismatched adapter,
    /// unsupported format, device lost, etc.).
    fn import_content_texture(
        &mut self,
        surface_id_packed: u64,
        source: Self::ContentTextureSource,
    ) -> Option<Self::TextureToken>;

    /// Release the imported texture for `surface_id_packed`. Subsequent calls
    /// to `import_content_texture` with the same id produce a fresh token.
    fn release_content_texture(&mut self, surface_id_packed: u64);

    /// Execute `f` with exclusive access to a command encoder bound to the
    /// current frame's queue submission. Ensures compositor UI passes and
    /// content composition share submit ordering.
    fn with_encoder<F: FnOnce(&mut Self::Encoder)>(&mut self, f: F);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_capabilities_fields_are_independent() {
        let import_only = HostGpuCapabilities {
            supports_shared_texture_import: true,
            supports_encoder_coordination: false,
        };
        let coord_only = HostGpuCapabilities {
            supports_shared_texture_import: false,
            supports_encoder_coordination: true,
        };
        assert_ne!(import_only, coord_only);
        assert!(import_only.supports_shared_texture_import);
        assert!(!import_only.supports_encoder_coordination);
        assert!(!coord_only.supports_shared_texture_import);
        assert!(coord_only.supports_encoder_coordination);
    }

    #[test]
    fn gpu_capabilities_both_on_or_off() {
        let both_on = HostGpuCapabilities {
            supports_shared_texture_import: true,
            supports_encoder_coordination: true,
        };
        let both_off = HostGpuCapabilities {
            supports_shared_texture_import: false,
            supports_encoder_coordination: false,
        };
        assert_ne!(both_on, both_off);
        assert_eq!(both_on, both_on);
    }
}
