/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use servo::{
    AuthenticationRequest, BluetoothDeviceSelectionRequest, EmbedderControl,
    EmbedderControlId, PermissionRequest, WebViewId,
};

use super::HeadedWindow;
use crate::shell::desktop::ui::dialog::Dialog;

pub(super) fn show_embedder_control(
    window: &HeadedWindow,
    webview_id: WebViewId,
    embedder_control: EmbedderControl,
) {
    let control_id = embedder_control.id();
    match embedder_control {
        EmbedderControl::SelectElement(prompt) => {
            let offset = window.gui.borrow().toolbar_height();
            window.add_dialog(
                webview_id,
                Dialog::new_select_element_dialog(prompt, offset),
            );
        }
        EmbedderControl::ColorPicker(color_picker) => {
            let offset = window.gui.borrow().toolbar_height();
            window.add_dialog(
                webview_id,
                Dialog::new_color_picker_dialog(color_picker, offset),
            );
        }
        EmbedderControl::InputMethod(input_method_control) => {
            if input_method_control.allow_virtual_keyboard() {
                window.visible_input_methods.borrow_mut().push(control_id);
                window.show_ime(input_method_control);
            }
        }
        EmbedderControl::FilePicker(file_picker) => {
            window.add_dialog(webview_id, Dialog::new_file_dialog(file_picker));
        }
        EmbedderControl::SimpleDialog(simple_dialog) => match simple_dialog {
            servo::SimpleDialog::Prompt(mut prompt_dialog) => {
                let bridge_response = {
                    let mut gui = window.gui.borrow_mut();
                    gui.try_handle_nip07_prompt(webview_id, prompt_dialog.message())
                };
                if let Some(response_json) = bridge_response {
                    prompt_dialog.set_current_value(&response_json);
                    prompt_dialog.confirm();
                } else {
                    window.add_dialog(
                        webview_id,
                        Dialog::new_simple_dialog(servo::SimpleDialog::Prompt(prompt_dialog)),
                    );
                }
            }
            other => window.add_dialog(webview_id, Dialog::new_simple_dialog(other)),
        },
        EmbedderControl::ContextMenu(prompt) => {
            let mut gui = window.gui.borrow_mut();
            let offset = gui.toolbar_height();
            let graphshell_anchor = [
                prompt.position().min.x as f32,
                (prompt.position().min.y + offset.0 as i32) as f32,
            ];
            if gui.node_key_for_webview_id(webview_id).is_some() {
                gui.request_context_command_surface_for_webview(webview_id, graphshell_anchor);
                drop(gui);
                window.winit_window.request_redraw();
            } else {
                drop(gui);
                window.add_dialog(
                    webview_id,
                    Dialog::new_context_menu(webview_id, prompt, offset),
                );
            }
        }
    }
}

pub(super) fn hide_embedder_control(
    window: &HeadedWindow,
    webview_id: WebViewId,
    embedder_control_id: EmbedderControlId,
) {
    {
        let mut visible_input_methods = window.visible_input_methods.borrow_mut();
        if let Some(index) = visible_input_methods
            .iter()
            .position(|visible_id| *visible_id == embedder_control_id)
        {
            visible_input_methods.remove(index);
            window.winit_window.set_ime_allowed(false);
        }
    }
    window.remove_dialog(webview_id, embedder_control_id);
}

pub(super) fn show_bluetooth_device_dialog(
    window: &HeadedWindow,
    webview_id: WebViewId,
    request: BluetoothDeviceSelectionRequest,
) {
    window.add_dialog(webview_id, Dialog::new_device_selection_dialog(request));
}

pub(super) fn show_permission_dialog(
    window: &HeadedWindow,
    webview_id: WebViewId,
    permission_request: PermissionRequest,
) {
    window.add_dialog(
        webview_id,
        Dialog::new_permission_request_dialog(permission_request),
    );
}

pub(super) fn show_http_authentication_dialog(
    window: &HeadedWindow,
    webview_id: WebViewId,
    authentication_request: AuthenticationRequest,
) {
    window.add_dialog(
        webview_id,
        Dialog::new_authentication_dialog(authentication_request),
    );
}

pub(super) fn dismiss_embedder_controls_for_webview(window: &HeadedWindow, webview_id: WebViewId) {
    window.dialogs.borrow_mut().remove(&webview_id);
}

