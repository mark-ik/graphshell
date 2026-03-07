/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shell::desktop::workbench::pane_model::TileRenderMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct InteractionUiState {
    pub(crate) interaction_menu_open: bool,
    pub(crate) help_panel_open: bool,
    pub(crate) radial_menu_open: bool,
}

impl InteractionUiState {
    pub(crate) fn new(
        interaction_menu_open: bool,
        help_panel_open: bool,
        radial_menu_open: bool,
    ) -> Self {
        Self {
            interaction_menu_open,
            help_panel_open,
            radial_menu_open,
        }
    }

    pub(crate) fn overlay_suppression_reason(self) -> Option<OverlaySuppressionReason> {
        if self.interaction_menu_open {
            Some(OverlaySuppressionReason::InteractionMenu)
        } else if self.help_panel_open {
            Some(OverlaySuppressionReason::HelpPanel)
        } else if self.radial_menu_open {
            Some(OverlaySuppressionReason::RadialMenu)
        } else {
            None
        }
    }

    pub(crate) fn native_overlay_visible(self) -> bool {
        self.overlay_suppression_reason().is_none()
    }

    pub(crate) fn effective_interaction_render_mode(
        self,
        base_mode: TileRenderMode,
    ) -> TileRenderMode {
        if base_mode == TileRenderMode::NativeOverlay && !self.native_overlay_visible() {
            TileRenderMode::Placeholder
        } else {
            base_mode
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverlaySuppressionReason {
    InteractionMenu,
    HelpPanel,
    RadialMenu,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppression_reason_uses_deterministic_priority() {
        let state = InteractionUiState::new(true, true, true);
        assert_eq!(
            state.overlay_suppression_reason(),
            Some(OverlaySuppressionReason::InteractionMenu)
        );

        let state = InteractionUiState::new(false, true, true);
        assert_eq!(
            state.overlay_suppression_reason(),
            Some(OverlaySuppressionReason::HelpPanel)
        );

        let state = InteractionUiState::new(false, false, true);
        assert_eq!(
            state.overlay_suppression_reason(),
            Some(OverlaySuppressionReason::RadialMenu)
        );
    }

    #[test]
    fn interaction_render_mode_falls_back_from_native_overlay_when_suppressed() {
        let suppressed = InteractionUiState::new(true, false, false);
        assert_eq!(
            suppressed.effective_interaction_render_mode(TileRenderMode::NativeOverlay),
            TileRenderMode::Placeholder
        );

        let clear = InteractionUiState::new(false, false, false);
        assert_eq!(
            clear.effective_interaction_render_mode(TileRenderMode::NativeOverlay),
            TileRenderMode::NativeOverlay
        );
    }
}
