/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced-backed host adapter — M5 skeleton.
//!
//! Sibling of [`super::gui::EguiHost`]. This module owns the iced-side
//! wiring of the shared `GraphshellRuntime`: its job is to translate
//! iced's native event stream into [`FrameHostInput`], call
//! `runtime.tick(&input, &mut IcedHostPorts)`, and drive iced's render
//! pipeline with the returned [`FrameViewModel`].
//!
//! **Status**: M5 step 1 — "Add an iced host behind a feature flag or
//! separate desktop entry point". The struct below exists, holds the
//! host-neutral runtime, and constructs `FrameHostInput` / `FrameViewModel`
//! types correctly. It does not yet render anything, and the event
//! translation / port wiring are placeholder. Subsequent M5 steps fill
//! those in.
//!
//! The `iced` crate is intentionally not yet a dependency — the skeleton
//! proves the trait bundle compiles for a second host shape without
//! pulling the full iced dependency tree. Adding iced as a real dependency
//! lands together with the first actual rendering pass.

use crate::shell::desktop::ui::frame_model::{FrameHostInput, FrameViewModel};
use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
use crate::shell::desktop::ui::iced_host_ports::IcedHostPorts;

/// iced-side host adapter around a shared `GraphshellRuntime`.
///
/// In the fully-wired M5 end state this struct will also hold iced-specific
/// render state (iced `Application` context, viewport state, texture cache,
/// accesskit bridge). M5 step 1 leaves it deliberately thin so the
/// host-neutral runtime can be stood up in isolation.
pub(crate) struct IcedHost {
    /// Host-neutral runtime state shared with `EguiHost`. The whole point
    /// of M3.5 / M4 was to extract this so two hosts can drive the same
    /// state machine.
    pub(crate) runtime: GraphshellRuntime,
}

impl IcedHost {
    /// Construct a minimal `IcedHost` for skeleton / test purposes.
    ///
    /// Uses `GraphshellRuntime::for_testing()` so this can run without a
    /// full iced application context. Real construction (with persistence
    /// paths, tokio runtime sharing, etc.) lands with the iced crate
    /// dependency.
    #[cfg(test)]
    pub(crate) fn new_for_testing() -> Self {
        Self {
            runtime: GraphshellRuntime::for_testing(),
        }
    }

    /// Drive one tick of the shared runtime.
    ///
    /// Placeholder: the real implementation will translate iced's per-frame
    /// inputs into `FrameHostInput` (mirroring `build_frame_host_input` in
    /// the egui host), call `runtime.tick`, and hand the returned
    /// `FrameViewModel` to iced's view function.
    ///
    /// Returned `FrameViewModel` is discarded during M5 bring-up because
    /// there's no iced renderer wired yet.
    pub(crate) fn tick_with_input(&mut self, input: &FrameHostInput) -> FrameViewModel {
        let mut ports = IcedHostPorts;
        self.runtime.tick(input, &mut ports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: the shared runtime can be driven through a second host
    /// shape. The egui host's test coverage exercises the runtime's behavior;
    /// this test only verifies the composition.
    #[test]
    fn iced_host_drives_runtime_tick() {
        let mut host = IcedHost::new_for_testing();
        let input = FrameHostInput::default();
        let _view_model = host.tick_with_input(&input);
    }
}
