/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::rc::Rc;

use servo::{GamepadHapticEffectType, InputEvent, WebViewId};

use crate::shell::desktop::host::gamepad::{AppGamepadProvider, GamepadDispatch, GamepadUiCommand};
use crate::shell::desktop::host::window::EmbedderWindow;

pub(crate) struct GamepadRuntime {
    provider: Option<Rc<AppGamepadProvider>>,
    pending_ui_commands: RefCell<Vec<GamepadUiCommand>>,
}

impl GamepadRuntime {
    pub(crate) fn new(provider: Option<Rc<AppGamepadProvider>>) -> Self {
        Self {
            provider,
            pending_ui_commands: Default::default(),
        }
    }

    pub(crate) fn handle_events(&self, focused_window: Option<Rc<EmbedderWindow>>) {
        let Some(provider) = self.provider.as_ref() else {
            return;
        };

        let focused_webview = focused_window.and_then(|window| {
            let webview_id = resolve_content_webview_id(&window)?;
            window.webview_by_id(webview_id)
        });

        for dispatch in provider.handle_gamepad_events() {
            match dispatch {
                GamepadDispatch::Ui(command) => {
                    self.pending_ui_commands.borrow_mut().push(command);
                }
                GamepadDispatch::Content(event) => {
                    if let Some(webview) = focused_webview.as_ref() {
                        webview.notify_input_event(InputEvent::Gamepad(event));
                    }
                }
            }
        }
    }

    pub(crate) fn take_pending_ui_commands(&self) -> Vec<GamepadUiCommand> {
        std::mem::take(&mut *self.pending_ui_commands.borrow_mut())
    }

    pub(crate) fn play_haptic_effect(
        &self,
        index: usize,
        effect_type: GamepadHapticEffectType,
        effect_complete_callback: Box<dyn FnOnce(bool)>,
    ) {
        match self.provider.as_ref() {
            Some(provider) => {
                provider.play_haptic_effect(index, effect_type, effect_complete_callback);
            }
            None => {
                effect_complete_callback(false);
            }
        }
    }

    pub(crate) fn stop_haptic_effect(
        &self,
        index: usize,
        haptic_stop_callback: Box<dyn FnOnce(bool)>,
    ) {
        let stopped = match self.provider.as_ref() {
            Some(provider) => provider.stop_haptic_effect(index),
            None => false,
        };
        haptic_stop_callback(stopped);
    }
}

pub(crate) fn resolve_content_webview_id(window: &EmbedderWindow) -> Option<WebViewId> {
    window.targeted_input_webview_id()
}

