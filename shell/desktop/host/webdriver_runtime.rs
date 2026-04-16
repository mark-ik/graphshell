/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;

use crossbeam_channel::{Receiver, Sender, unbounded};
use euclid::Rect;
use image::RgbaImage;
use log::{info, warn};
use servo::{
    CSSPixel, EmbedderControl, EmbedderControlId, EventLoopWaker, GenericSender, InputEvent,
    InputEventId, JSValue, NewWindowTypeHint, Preferences, ScreenshotCaptureError,
    WebDriverCommandMsg, WebDriverJSResult, WebDriverLoadStatus, WebDriverScriptCommand,
    WebDriverSenders, WebDriverUserPrompt, WebDriverUserPromptAction, WebViewId,
};
use url::Url;

use crate::app::BrowserCommand;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_MISSING_WEBVIEW,
    CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_REQUESTED, CHANNEL_HOST_WEBDRIVER_LOAD_STATUS_BLOCKED,
    CHANNEL_HOST_WEBDRIVER_LOAD_URL_MISSING_WEBVIEW, CHANNEL_HOST_WEBDRIVER_LOAD_URL_REQUESTED,
};
#[cfg(feature = "ux-bridge")]
use crate::shell::desktop::workbench::ux_bridge;

use super::running_app_state::RunningAppState;
use super::window::PlatformWindow;

const WEBDRIVER_LOAD_URL_LABEL: &str = "load_url";

fn emit_webdriver_browser_action_requested(command: BrowserCommand) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_REQUESTED,
        byte_len: command.diagnostic_label().len(),
    });
}

fn emit_webdriver_browser_action_missing_webview(command: BrowserCommand) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_MISSING_WEBVIEW,
        byte_len: command.diagnostic_label().len(),
    });
}

fn emit_webdriver_load_status_blocked() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_HOST_WEBDRIVER_LOAD_STATUS_BLOCKED,
        byte_len: "blocked".len(),
    });
}

fn emit_webdriver_load_url_requested() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_HOST_WEBDRIVER_LOAD_URL_REQUESTED,
        byte_len: WEBDRIVER_LOAD_URL_LABEL.len(),
    });
}

fn emit_webdriver_load_url_missing_webview() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_HOST_WEBDRIVER_LOAD_URL_MISSING_WEBVIEW,
        byte_len: WEBDRIVER_LOAD_URL_LABEL.len(),
    });
}

fn send_webdriver_load_status_blocked(sender: &GenericSender<WebDriverLoadStatus>) {
    emit_webdriver_load_status_blocked();
    let _ = sender.send(WebDriverLoadStatus::Blocked);
}

#[derive(Default)]
pub(crate) struct WebDriverEmbedderControls {
    embedder_controls: RefCell<HashMap<WebViewId, Vec<EmbedderControl>>>,
}

impl WebDriverEmbedderControls {
    pub(crate) fn show_embedder_control(
        &self,
        webview_id: WebViewId,
        embedder_control: EmbedderControl,
    ) {
        self.embedder_controls
            .borrow_mut()
            .entry(webview_id)
            .or_default()
            .push(embedder_control)
    }

    pub(crate) fn hide_embedder_control(
        &self,
        webview_id: WebViewId,
        embedder_control_id: EmbedderControlId,
    ) {
        let mut embedder_controls = self.embedder_controls.borrow_mut();
        if let Some(controls) = embedder_controls.get_mut(&webview_id) {
            controls.retain(|control| control.id() != embedder_control_id);
        }
        embedder_controls.retain(|_, controls| !controls.is_empty());
    }

    pub(crate) fn current_active_dialog_webdriver_type(
        &self,
        webview_id: WebViewId,
    ) -> Option<WebDriverUserPrompt> {
        let embedder_controls = self.embedder_controls.borrow();
        match embedder_controls.get(&webview_id)?.last()? {
            EmbedderControl::SimpleDialog(servo::SimpleDialog::Alert(..)) => {
                Some(WebDriverUserPrompt::Alert)
            }
            EmbedderControl::SimpleDialog(servo::SimpleDialog::Confirm(..)) => {
                Some(WebDriverUserPrompt::Confirm)
            }
            EmbedderControl::SimpleDialog(servo::SimpleDialog::Prompt(..)) => {
                Some(WebDriverUserPrompt::Prompt)
            }
            EmbedderControl::FilePicker { .. } => Some(WebDriverUserPrompt::File),
            EmbedderControl::SelectElement { .. } => Some(WebDriverUserPrompt::Default),
            _ => None,
        }
    }

    pub(crate) fn respond_to_active_simple_dialog(
        &self,
        webview_id: WebViewId,
        action: WebDriverUserPromptAction,
    ) -> Result<String, ()> {
        let mut embedder_controls = self.embedder_controls.borrow_mut();
        let Some(controls) = embedder_controls.get_mut(&webview_id) else {
            return Err(());
        };
        let Some(&EmbedderControl::SimpleDialog(simple_dialog)) = controls.last().as_ref() else {
            return Err(());
        };

        let result_text = simple_dialog.message().to_owned();
        if action == WebDriverUserPromptAction::Ignore {
            return Ok(result_text);
        }

        let Some(EmbedderControl::SimpleDialog(simple_dialog)) = controls.pop() else {
            return Err(());
        };
        match action {
            WebDriverUserPromptAction::Accept => simple_dialog.confirm(),
            WebDriverUserPromptAction::Dismiss => simple_dialog.dismiss(),
            WebDriverUserPromptAction::Ignore => unreachable!("Should have returned early above"),
        }
        Ok(result_text)
    }

    pub(crate) fn message_of_newest_dialog(&self, webview_id: WebViewId) -> Option<String> {
        let embedder_controls = self.embedder_controls.borrow();
        match embedder_controls.get(&webview_id)?.last()? {
            EmbedderControl::SimpleDialog(simple_dialog) => Some(simple_dialog.message().into()),
            _ => None,
        }
    }

    pub(crate) fn set_prompt_value_of_newest_dialog(&self, webview_id: WebViewId, text: String) {
        let mut embedder_controls = self.embedder_controls.borrow_mut();
        let Some(controls) = embedder_controls.get_mut(&webview_id) else {
            return;
        };
        let Some(&mut EmbedderControl::SimpleDialog(servo::SimpleDialog::Prompt(
            ref mut prompt_dialog,
        ))) = controls.last_mut()
        else {
            return;
        };
        prompt_dialog.set_current_value(&text);
    }
}

pub(crate) struct WebDriverRuntime {
    senders: RefCell<WebDriverSenders>,
    embedder_controls: WebDriverEmbedderControls,
    pending_events: RefCell<HashMap<InputEventId, Sender<()>>>,
    receiver: Option<Receiver<WebDriverCommandMsg>>,
}

impl WebDriverRuntime {
    pub(crate) fn new(
        port: Option<u16>,
        event_loop_waker: Box<dyn EventLoopWaker>,
        default_preferences: Preferences,
    ) -> Self {
        let receiver = port.map(|port| {
            let (embedder_sender, embedder_receiver) = unbounded();
            webdriver_server::start_server(
                port,
                embedder_sender,
                event_loop_waker,
                default_preferences,
            );
            embedder_receiver
        });

        Self {
            senders: RefCell::default(),
            embedder_controls: Default::default(),
            pending_events: RefCell::default(),
            receiver,
        }
    }

    pub(crate) fn handle_messages(
        &self,
        state: &Rc<RunningAppState>,
        create_platform_window: Option<&dyn Fn(Url) -> Rc<dyn PlatformWindow>>,
    ) {
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };

        while let Ok(msg) = receiver.try_recv() {
            match msg {
                WebDriverCommandMsg::ResetAllCookies(sender) => {
                    state.servo().site_data_manager().clear_cookies();
                    let _ = sender.send(());
                }
                WebDriverCommandMsg::Shutdown => {
                    state.schedule_exit();
                }
                WebDriverCommandMsg::IsWebViewOpen(webview_id, sender) => {
                    if let Err(error) = sender.send(state.webview_by_id(webview_id).is_some()) {
                        warn!("Failed to send response of IsWebViewOpen: {error}");
                    }
                }
                WebDriverCommandMsg::IsBrowsingContextOpen(..)
                | WebDriverCommandMsg::FocusBrowsingContext(..) => {
                    state.servo().execute_webdriver_command(msg);
                }
                WebDriverCommandMsg::NewWindow(type_hint, response_sender, load_status_sender) => {
                    let url = Url::parse("about:blank").unwrap();
                    let new_webview = match (type_hint, create_platform_window) {
                        (
                            NewWindowTypeHint::Window | NewWindowTypeHint::Auto,
                            Some(create_platform_window),
                        ) => {
                            let window =
                                state.open_window(create_platform_window(url.clone()), url);
                            window
                                .explicit_input_webview_id()
                                .and_then(|id| window.webview_by_id(id))
                                .expect("Should have at last one WebView in new window")
                        }
                        _ => state
                            .windows()
                            .values()
                            .nth(0)
                            .expect("Expected at least one window to be open")
                            .create_toplevel_webview(state.clone(), url),
                    };

                    if let Err(error) = response_sender.send(new_webview.id()) {
                        warn!("Failed to send response of NewWebview: {error}");
                    }
                    if let Some(load_status_sender) = load_status_sender {
                        self.set_load_status_sender(new_webview.id(), load_status_sender);
                    }
                }
                WebDriverCommandMsg::CloseWebView(webview_id, response_sender) => {
                    emit_webdriver_browser_action_requested(BrowserCommand::Close);
                    if let Some(window) = state.maybe_window_for_webview_id(webview_id) {
                        window.close_webview(webview_id);
                    } else {
                        emit_webdriver_browser_action_missing_webview(BrowserCommand::Close);
                    }
                    if let Err(error) = response_sender.send(()) {
                        warn!("Failed to send response of CloseWebView: {error}");
                    }
                }
                WebDriverCommandMsg::FocusWebView(webview_id) => {
                    let window = state.window_for_webview_id(webview_id);
                    window.retarget_input_to_webview(webview_id);
                    state.focus_window(window);
                }
                WebDriverCommandMsg::GetAllWebViews(response_sender) => {
                    let webviews = state
                        .windows()
                        .values()
                        .flat_map(|window| window.webview_ids())
                        .collect();
                    if let Err(error) = response_sender.send(webviews) {
                        warn!("Failed to send response of GetAllWebViews: {error}");
                    }
                }
                WebDriverCommandMsg::GetWindowRect(webview_id, response_sender) => {
                    let platform_window = state.platform_window_for_webview_id(webview_id);
                    if let Err(error) = response_sender.send(platform_window.window_rect()) {
                        warn!("Failed to send response of GetWindowSize: {error}");
                    }
                }
                WebDriverCommandMsg::MaximizeWebView(webview_id, response_sender) => {
                    let Some(webview) = state.webview_by_id(webview_id) else {
                        continue;
                    };
                    let platform_window = state.platform_window_for_webview_id(webview_id);
                    platform_window.maximize(&webview);

                    if let Err(error) = response_sender.send(platform_window.window_rect()) {
                        warn!("Failed to send response of GetWindowSize: {error}");
                    }
                }
                WebDriverCommandMsg::SetWindowRect(webview_id, requested_rect, size_sender) => {
                    let Some(webview) = state.webview_by_id(webview_id) else {
                        continue;
                    };

                    let platform_window = state.platform_window_for_webview_id(webview_id);
                    let scale = platform_window.hidpi_scale_factor();
                    let requested_physical_rect =
                        (requested_rect.to_f32() * scale).round().to_i32();

                    platform_window.request_resize(&webview, requested_physical_rect.size());
                    platform_window.set_position(requested_physical_rect.min);

                    if let Err(error) = size_sender.send(platform_window.window_rect()) {
                        warn!("Failed to send window size: {error}");
                    }
                }
                WebDriverCommandMsg::GetViewportSize(webview_id, response_sender) => {
                    let platform_window = state.platform_window_for_webview_id(webview_id);
                    let size = platform_window.rendering_context().size2d().to_f32()
                        / platform_window.hidpi_scale_factor();
                    if let Err(error) = response_sender.send(size) {
                        warn!("Failed to send response of GetViewportSize: {error}");
                    }
                }
                WebDriverCommandMsg::GetFocusedWebView(sender) => {
                    let preferred_input_webview = state.focused_window().and_then(|window| {
                        let webview_id = window.explicit_input_webview_id()?;
                        window.webview_by_id(webview_id).map(|webview| webview.id())
                    });
                    if let Err(error) = sender.send(preferred_input_webview) {
                        warn!("Failed to send response of GetFocusedWebView: {error}");
                    }
                }
                WebDriverCommandMsg::LoadUrl(webview_id, url, load_status_sender) => {
                    self.handle_load_url(state, webview_id, url, load_status_sender);
                }
                WebDriverCommandMsg::Refresh(webview_id, load_status_sender) => {
                    emit_webdriver_browser_action_requested(BrowserCommand::Reload);
                    if let Some(webview) = state.webview_by_id(webview_id) {
                        self.set_load_status_sender(webview_id, load_status_sender);
                        webview.reload();
                    } else {
                        emit_webdriver_browser_action_missing_webview(BrowserCommand::Reload);
                        send_webdriver_load_status_blocked(&load_status_sender);
                    }
                }
                WebDriverCommandMsg::GoBack(webview_id, load_status_sender) => {
                    emit_webdriver_browser_action_requested(BrowserCommand::Back);
                    if let Some(webview) = state.webview_by_id(webview_id) {
                        let traversal_id = webview.go_back(1);
                        self.set_pending_traversal(traversal_id, load_status_sender);
                    } else {
                        emit_webdriver_browser_action_missing_webview(BrowserCommand::Back);
                        send_webdriver_load_status_blocked(&load_status_sender);
                    }
                }
                WebDriverCommandMsg::GoForward(webview_id, load_status_sender) => {
                    emit_webdriver_browser_action_requested(BrowserCommand::Forward);
                    if let Some(webview) = state.webview_by_id(webview_id) {
                        let traversal_id = webview.go_forward(1);
                        self.set_pending_traversal(traversal_id, load_status_sender);
                    } else {
                        emit_webdriver_browser_action_missing_webview(BrowserCommand::Forward);
                        send_webdriver_load_status_blocked(&load_status_sender);
                    }
                }
                WebDriverCommandMsg::InputEvent(webview_id, input_event, response_sender) => {
                    self.handle_input_event(state, webview_id, input_event, response_sender);
                }
                WebDriverCommandMsg::ScriptCommand(_, ref webdriver_script_command) => {
                    #[cfg(feature = "ux-bridge")]
                    if self.try_handle_ux_bridge_script_command(state, webdriver_script_command) {
                        continue;
                    }
                    self.handle_script_command(webdriver_script_command);
                    state.servo().execute_webdriver_command(msg);
                }
                WebDriverCommandMsg::CurrentUserPrompt(webview_id, response_sender) => {
                    let current_dialog = self
                        .embedder_controls
                        .current_active_dialog_webdriver_type(webview_id);
                    if let Err(error) = response_sender.send(current_dialog) {
                        warn!("Failed to send response of CurrentUserPrompt: {error}");
                    }
                }
                WebDriverCommandMsg::HandleUserPrompt(webview_id, action, response_sender) => {
                    let result = self
                        .embedder_controls
                        .respond_to_active_simple_dialog(webview_id, action);
                    if let Err(error) = response_sender.send(result) {
                        warn!("Failed to send response of HandleUserPrompt: {error}");
                    }
                }
                WebDriverCommandMsg::GetAlertText(webview_id, response_sender) => {
                    let response = match self.embedder_controls.message_of_newest_dialog(webview_id)
                    {
                        Some(text) => Ok(text),
                        None => Err(()),
                    };

                    if let Err(error) = response_sender.send(response) {
                        warn!("Failed to send response of GetAlertText: {error}");
                    }
                }
                WebDriverCommandMsg::SendAlertText(webview_id, text) => {
                    self.embedder_controls
                        .set_prompt_value_of_newest_dialog(webview_id, text);
                }
                WebDriverCommandMsg::TakeScreenshot(webview_id, rect, result_sender) => {
                    self.handle_screenshot(state, webview_id, rect, result_sender);
                }
            }
        }
    }

    pub(crate) fn set_pending_traversal(
        &self,
        traversal_id: servo::TraversalId,
        sender: GenericSender<WebDriverLoadStatus>,
    ) {
        self.senders
            .borrow_mut()
            .pending_traversals
            .insert(traversal_id, sender);
    }

    pub(crate) fn set_load_status_sender(
        &self,
        webview_id: WebViewId,
        sender: GenericSender<WebDriverLoadStatus>,
    ) {
        self.senders
            .borrow_mut()
            .load_status_senders
            .insert(webview_id, sender);
    }

    fn remove_load_status_sender(&self, webview_id: WebViewId) {
        self.senders
            .borrow_mut()
            .load_status_senders
            .remove(&webview_id);
    }

    fn set_script_command_interrupt_sender(
        &self,
        sender: Option<GenericSender<WebDriverJSResult>>,
    ) {
        self.senders.borrow_mut().script_evaluation_interrupt_sender = sender;
    }

    fn handle_input_event(
        &self,
        state: &RunningAppState,
        webview_id: WebViewId,
        input_event: InputEvent,
        response_sender: Option<Sender<()>>,
    ) {
        if let Some(webview) = state.webview_by_id(webview_id) {
            let event_id = webview.notify_input_event(input_event);
            if let Some(response_sender) = response_sender {
                self.pending_events
                    .borrow_mut()
                    .insert(event_id, response_sender);
            }
        } else {
            log::error!(
                "Could not find WebView ({webview_id:?}) for WebDriver event: {input_event:?}"
            );
        }
    }

    fn handle_screenshot(
        &self,
        state: &RunningAppState,
        webview_id: WebViewId,
        rect: Option<Rect<f32, CSSPixel>>,
        result_sender: Sender<Result<RgbaImage, ScreenshotCaptureError>>,
    ) {
        if let Some(webview) = state.webview_by_id(webview_id) {
            let rect = rect.map(|rect| rect.to_box2d().into());
            webview.take_screenshot(rect, move |result| {
                if let Err(error) = result_sender.send(result) {
                    warn!("Failed to send response to TakeScreenshot: {error}");
                }
            });
        } else if let Err(error) =
            result_sender.send(Err(ScreenshotCaptureError::WebViewDoesNotExist))
        {
            log::error!("Failed to send response to TakeScreenshot: {error}");
        }
    }

    fn handle_script_command(&self, script_command: &WebDriverScriptCommand) {
        match script_command {
            WebDriverScriptCommand::ExecuteScriptWithCallback(_webview_id, response_sender) => {
                self.set_script_command_interrupt_sender(Some(response_sender.clone()));
            }
            WebDriverScriptCommand::AddLoadStatusSender(webview_id, load_status_sender) => {
                self.set_load_status_sender(*webview_id, load_status_sender.clone());
            }
            WebDriverScriptCommand::RemoveLoadStatusSender(webview_id) => {
                self.remove_load_status_sender(*webview_id);
            }
            _ => {
                self.set_script_command_interrupt_sender(None);
            }
        }
    }

    #[cfg(feature = "ux-bridge")]
    fn try_handle_ux_bridge_script_command(
        &self,
        state: &RunningAppState,
        script_command: &WebDriverScriptCommand,
    ) -> bool {
        let WebDriverScriptCommand::ExecuteScriptWithCallback(script, response_sender) =
            script_command
        else {
            return false;
        };

        let Some(payload) = script.strip_prefix(ux_bridge::WEBDRIVER_SCRIPT_PREFIX) else {
            return false;
        };

        let result = handle_ux_bridge_script_payload(payload, |intent| {
            let window = state
                .focused_window()
                .or_else(|| state.windows().values().next().cloned())
                .ok_or_else(|| {
                    ux_bridge::UxBridgeError::transport_unavailable(
                        "No embedder window is available for queued ux bridge actions.",
                    )
                })?;
            window.notify_webdriver_workbench_intent_request(intent);
            Ok(())
        });

        let _ = response_sender.send(Ok(json_value_to_js_value(&result)));
        true
    }

    fn handle_load_url(
        &self,
        state: &RunningAppState,
        webview_id: WebViewId,
        url: Url,
        load_status_sender: GenericSender<WebDriverLoadStatus>,
    ) {
        emit_webdriver_load_url_requested();
        let Some(webview) = state.webview_by_id(webview_id) else {
            emit_webdriver_load_url_missing_webview();
            send_webdriver_load_status_blocked(&load_status_sender);
            return;
        };

        state
            .platform_window_for_webview_id(webview_id)
            .dismiss_embedder_controls_for_webview(webview_id);

        info!("Loading URL in webview {}: {}", webview_id, url);
        self.set_load_status_sender(webview_id, load_status_sender);
        webview.load(url.into());
    }

    pub(crate) fn interrupt_script_evaluation(&self) {
        if let Some(sender) = &self.senders.borrow().script_evaluation_interrupt_sender {
            sender.send(Ok(JSValue::Null)).unwrap_or_else(|err| {
                info!(
                    "Notify dialog appear failed. Maybe the channel to webdriver is closed: {err}"
                );
            });
        }
    }

    pub(crate) fn complete_traversal(&self, traversal_id: servo::TraversalId) {
        let mut webdriver_state = self.senders.borrow_mut();
        if let Entry::Occupied(entry) = webdriver_state.pending_traversals.entry(traversal_id) {
            let sender = entry.remove();
            let _ = sender.send(WebDriverLoadStatus::Complete);
        }
    }

    pub(crate) fn finish_input_event(&self, id: InputEventId) {
        if let Some(response_sender) = self.pending_events.borrow_mut().remove(&id) {
            let _ = response_sender.send(());
        }
    }

    pub(crate) fn take_load_status_sender(
        &self,
        webview_id: WebViewId,
    ) -> Option<GenericSender<WebDriverLoadStatus>> {
        self.senders
            .borrow_mut()
            .load_status_senders
            .remove(&webview_id)
    }

    pub(crate) fn block_load_status_if_any(&self, webview_id: WebViewId) {
        if let Some(sender) = self
            .senders
            .borrow_mut()
            .load_status_senders
            .get(&webview_id)
        {
            send_webdriver_load_status_blocked(sender);
        }
    }

    pub(crate) fn show_embedder_control(
        &self,
        webview_id: WebViewId,
        embedder_control: EmbedderControl,
    ) {
        self.embedder_controls
            .show_embedder_control(webview_id, embedder_control);
    }

    pub(crate) fn hide_embedder_control(
        &self,
        webview_id: WebViewId,
        embedder_control_id: EmbedderControlId,
    ) {
        self.embedder_controls
            .hide_embedder_control(webview_id, embedder_control_id);
    }
}

#[cfg(feature = "ux-bridge")]
fn handle_ux_bridge_script_payload<F>(payload: &str, enqueue_intent: F) -> serde_json::Value
where
    F: FnOnce(crate::app::WorkbenchIntent) -> Result<(), ux_bridge::UxBridgeError>,
{
    let command = match ux_bridge::parse_transport_command(payload) {
        Ok(command) => command,
        Err(error) => return ux_bridge::error_json(&error),
    };

    match command {
        ux_bridge::UxBridgeCommand::GetUxSnapshot
        | ux_bridge::UxBridgeCommand::FindUxNode { .. }
        | ux_bridge::UxBridgeCommand::GetFocusPath => {
            match ux_bridge::handle_latest_snapshot_command(command) {
                Ok(response) => ux_bridge::response_json(&response),
                Err(error) => ux_bridge::error_json(&error),
            }
        }
        ux_bridge::UxBridgeCommand::InvokeUxAction { selector, action } => {
            let (intent, response) =
                match ux_bridge::queued_workbench_intent_for_latest_snapshot(&selector, action) {
                    Ok(result) => result,
                    Err(error) => return ux_bridge::error_json(&error),
                };
            match enqueue_intent(intent) {
                Ok(()) => ux_bridge::response_json(&response),
                Err(error) => ux_bridge::error_json(&error),
            }
        }
    }
}

#[cfg(feature = "ux-bridge")]
fn json_value_to_js_value(value: &serde_json::Value) -> JSValue {
    match value {
        serde_json::Value::Null => JSValue::Null,
        serde_json::Value::Bool(value) => JSValue::Boolean(*value),
        serde_json::Value::Number(value) => JSValue::Number(value.as_f64().unwrap_or_default()),
        serde_json::Value::String(value) => JSValue::String(value.clone()),
        serde_json::Value::Array(values) => {
            JSValue::Array(values.iter().map(json_value_to_js_value).collect())
        }
        serde_json::Value::Object(entries) => JSValue::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), json_value_to_js_value(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, install_global_sender};
    #[cfg(feature = "ux-bridge")]
    use crate::shell::desktop::tests::harness::TestRegistry;
    #[cfg(feature = "ux-bridge")]
    use crate::shell::desktop::ui::toolbar::toolbar_ui::{
        CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
        CommandSurfaceSemanticSnapshot, clear_command_surface_semantic_snapshot,
        lock_command_surface_snapshot_tests, publish_command_surface_semantic_snapshot,
    };

    #[test]
    fn webdriver_browser_action_helpers_emit_expected_diagnostics() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);

        emit_webdriver_browser_action_requested(BrowserCommand::Reload);
        emit_webdriver_browser_action_missing_webview(BrowserCommand::Close);
        emit_webdriver_load_status_blocked();

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_REQUESTED
            )),
            "expected requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_MISSING_WEBVIEW
            )),
            "expected missing-webview diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_HOST_WEBDRIVER_LOAD_STATUS_BLOCKED
            )),
            "expected blocked-load diagnostic; got: {emitted:?}"
        );
    }

    #[test]
    fn webdriver_load_url_helpers_emit_expected_diagnostics() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);

        emit_webdriver_load_url_requested();
        emit_webdriver_load_url_missing_webview();

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_HOST_WEBDRIVER_LOAD_URL_REQUESTED
            )),
            "expected load-url requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_HOST_WEBDRIVER_LOAD_URL_MISSING_WEBVIEW
            )),
            "expected load-url missing-webview diagnostic; got: {emitted:?}"
        );
    }

    #[cfg(feature = "ux-bridge")]
    #[test]
    fn ux_bridge_query_script_reports_missing_snapshot() {
        crate::shell::desktop::workbench::ux_tree::clear_snapshot();

        let response = handle_ux_bridge_script_payload(r#"{"command":"GetUxSnapshot"}"#, |_| {
            panic!("query path should not enqueue workbench intents")
        });

        assert_eq!(
            response["error"]["kind"],
            serde_json::Value::String("SnapshotUnavailable".to_string())
        );
    }

    #[cfg(feature = "ux-bridge")]
    #[test]
    fn ux_bridge_action_script_queues_open_command_palette() {
        let _guard = lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            ..CommandSurfaceSemanticSnapshot::default()
        });

        let harness = TestRegistry::new();
        let snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            0,
        );
        crate::shell::desktop::workbench::ux_tree::publish_snapshot(&snapshot);

        let queued = std::cell::RefCell::new(None);
        let response = handle_ux_bridge_script_payload(
            ux_bridge::UxDriver::invoke_ux_action_script(
                &ux_bridge::UxNodeSelector::ByRole(
                    crate::shell::desktop::workbench::ux_tree::UxNodeRole::CommandBar,
                ),
                crate::shell::desktop::workbench::ux_tree::UxAction::Open,
            )
            .strip_prefix(ux_bridge::WEBDRIVER_SCRIPT_PREFIX)
            .expect("driver should emit the reserved webdriver prefix"),
            |intent| {
                queued.replace(Some(intent));
                Ok(())
            },
        );

        assert_eq!(response["ok"], serde_json::Value::Bool(true));
        assert_eq!(
            response["response"]["status"],
            serde_json::Value::String("Queued".to_string())
        );
        assert!(matches!(
            queued.into_inner(),
            Some(crate::app::WorkbenchIntent::OpenCommandPalette)
        ));

        clear_command_surface_semantic_snapshot();
    }

    #[cfg(feature = "ux-bridge")]
    #[test]
    fn ux_bridge_action_script_queues_node_pane_dismiss() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://webdriver-ux-bridge-node.example");
        harness.open_node_tab(node);

        let snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            0,
        );
        crate::shell::desktop::workbench::ux_tree::publish_snapshot(&snapshot);

        let node_pane = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                matches!(
                    entry.domain,
                    crate::shell::desktop::workbench::ux_tree::UxDomainIdentity::Node {
                        node_key,
                        pane_id: Some(_),
                        ..
                    } if node_key == node
                )
            })
            .expect("snapshot should include node pane semantic node");

        let queued = std::cell::RefCell::new(None);
        let response = handle_ux_bridge_script_payload(
            ux_bridge::UxDriver::invoke_ux_action_script(
                &ux_bridge::UxNodeSelector::ById(node_pane.ux_node_id.clone()),
                crate::shell::desktop::workbench::ux_tree::UxAction::Close,
            )
            .strip_prefix(ux_bridge::WEBDRIVER_SCRIPT_PREFIX)
            .expect("driver should emit the reserved webdriver prefix"),
            |intent| {
                queued.replace(Some(intent));
                Ok(())
            },
        );

        assert_eq!(response["ok"], serde_json::Value::Bool(true));
        assert_eq!(
            response["response"]["status"],
            serde_json::Value::String("Queued".to_string())
        );
        assert!(matches!(
            queued.into_inner(),
            Some(crate::app::WorkbenchIntent::DismissTile { .. })
        ));
    }
}
