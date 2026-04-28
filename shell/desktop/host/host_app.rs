/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Application entry point, runs the event loop.

use std::path::Path;
use std::rc::Rc;
use std::time::Instant;
use std::{env, fs};

use servo::{
    EventLoopWaker, Opts, Preferences, ServoBuilder, ServoUrl, UserContentManager, UserScript,
};
use url::Url;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::WindowId;

use super::event_loop::AppEvent;
use crate::parser::get_default_url;
use crate::prefs::AppPreferences;
use crate::registries::infrastructure::mod_loader::runtime_has_capability;
use crate::shell::desktop::host::event_loop::AppEventLoop;
use crate::shell::desktop::host::headed_window::HeadedWindow;
use crate::shell::desktop::host::headless_window::HeadlessWindow;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::{EmbedderWindowId, PlatformWindow};
use crate::shell::desktop::runtime::nip07_bridge;
use crate::shell::desktop::runtime::tracing::trace_winit_event;

const BUILTIN_USERSCRIPTS_DIRECTORY: &str = "resources/user-agent-js";

pub(crate) enum AppState {
    Initializing,
    Running(Rc<RunningAppState>),
    ShuttingDown,
}

pub struct App {
    opts: Opts,
    preferences: Preferences,
    app_preferences: AppPreferences,
    waker: Box<dyn EventLoopWaker>,
    event_loop_proxy: Option<EventLoopProxy<AppEvent>>,
    initial_url: ServoUrl,
    t_start: Instant,
    t: Instant,
    state: AppState,
}

impl App {
    pub fn new(
        opts: Opts,
        preferences: Preferences,
        app_preferences: AppPreferences,
        event_loop: &AppEventLoop,
    ) -> Self {
        let initial_url = get_default_url(
            app_preferences.url.as_deref(),
            env::current_dir().unwrap(),
            |path| fs::metadata(path).is_ok(),
            &app_preferences,
        );

        let t = Instant::now();
        App {
            opts,
            preferences,
            app_preferences,
            waker: event_loop.create_event_loop_waker(),
            event_loop_proxy: event_loop.event_loop_proxy(),
            initial_url: initial_url.clone(),
            t_start: t,
            t,
            state: AppState::Initializing,
        }
    }

    /// Initialize Application once event loop start running.
    pub fn init(&mut self, active_event_loop: Option<&ActiveEventLoop>) {
        let servo_builder = ServoBuilder::default()
            .opts(self.opts.clone())
            .preferences(self.preferences.clone())
            .protocol_registry(servo::protocol_handler::ProtocolRegistry::default())
            .event_loop_waker(self.waker.clone());

        let url = self.initial_url.as_url().clone();
        let platform_window = self.create_platform_window(url, active_event_loop);

        let servo = servo_builder.build();
        servo.setup_logging();

        let user_content_manager = Rc::new(UserContentManager::new(&servo));
        for user_stylesheet in &self.app_preferences.user_stylesheets {
            user_content_manager.add_stylesheet(user_stylesheet.clone());
        }
        if runtime_has_capability("nostr:nip07-bridge") {
            user_content_manager.add_script(Rc::new(UserScript::new(
                nip07_bridge::builtin_userscript_source().to_string(),
                None,
            )));
        }
        for script in load_userscripts(self.app_preferences.userscripts_directory.as_deref())
            .expect("Loading userscripts failed")
        {
            user_content_manager.add_script(script);
        }

        let running_state = Rc::new(RunningAppState::new(
            servo,
            self.app_preferences.clone(),
            self.waker.clone(),
            user_content_manager,
            self.preferences.clone(),
        ));
        running_state.open_window(platform_window, self.initial_url.as_url().clone());

        self.state = AppState::Running(running_state);
    }

    fn create_platform_window(
        &self,
        url: Url,
        active_event_loop: Option<&ActiveEventLoop>,
    ) -> Rc<dyn PlatformWindow> {
        assert_eq!(self.app_preferences.headless, active_event_loop.is_none());

        let Some(active_event_loop) = active_event_loop else {
            return HeadlessWindow::new(&self.app_preferences);
        };

        HeadedWindow::new(
            &self.app_preferences,
            active_event_loop,
            self.event_loop_proxy
                .clone()
                .expect("Should always have event loop proxy in headed mode."),
            url,
        )
    }

    pub fn pump_servo_event_loop(&mut self, active_event_loop: Option<&ActiveEventLoop>) -> bool {
        let AppState::Running(state) = &self.state else {
            return false;
        };

        let create_platform_window = |url: Url| self.create_platform_window(url, active_event_loop);
        if !state.spin_event_loop(Some(&create_platform_window)) {
            self.state = AppState::ShuttingDown;
            return false;
        }
        true
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.init(Some(event_loop));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        window_event: WindowEvent,
    ) {
        let now = Instant::now();
        trace_winit_event!(
            window_event,
            "@{:?} (+{:?}) {window_event:?}",
            now - self.t_start,
            now - self.t
        );
        self.t = now;

        let AppState::Running(state) = &self.state else {
            return;
        };

        if let Some(window) = state.window(EmbedderWindowId::from(u64::from(window_id))) {
            if let Some(headed_window) = window.platform_window().as_headed_window() {
                headed_window.handle_winit_window_event(state.clone(), window, window_event);
            }
        }

        if !self.pump_servo_event_loop(event_loop.into()) {
            event_loop.exit();
        }
        // Block until the window gets an event
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, app_event: AppEvent) {
        let AppState::Running(state) = &self.state else {
            return;
        };

        if let Some(window) = app_event
            .window_id()
            .and_then(|window_id| state.window(EmbedderWindowId::from(u64::from(window_id))))
        {
            if let Some(headed_window) = window.platform_window().as_headed_window() {
                headed_window.handle_winit_app_event(&window, app_event);
            }
        }

        if !self.pump_servo_event_loop(event_loop.into()) {
            event_loop.exit();
        }

        // Block until the window gets an event
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}

fn load_userscripts(userscripts_directory: Option<&Path>) -> std::io::Result<Vec<Rc<UserScript>>> {
    let mut userscripts = Vec::new();
    if let Some(userscripts_directory) = &userscripts_directory {
        for file in userscript_files(userscripts_directory)? {
            userscripts.push(Rc::new(UserScript::new(
                std::fs::read_to_string(&file)?,
                Some(file),
            )));
        }
    }
    Ok(userscripts)
}

fn userscript_files(userscripts_directory: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let resolved_directory = resolve_userscripts_directory(userscripts_directory);
    let mut files = std::fs::read_dir(resolved_directory)?
        .map(|e| e.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| path.is_file());
    files.sort_unstable();
    Ok(files)
}

fn resolve_userscripts_directory(userscripts_directory: &Path) -> std::path::PathBuf {
    if userscripts_directory == Path::new(BUILTIN_USERSCRIPTS_DIRECTORY) {
        crate::resources::resources_dir_path().join("user-agent-js")
    } else {
        userscripts_directory.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::{BUILTIN_USERSCRIPTS_DIRECTORY, resolve_userscripts_directory, userscript_files};
    use std::path::Path;

    #[test]
    fn resolves_builtin_userscripts_directory_via_resources_path() {
        let resolved = resolve_userscripts_directory(Path::new(BUILTIN_USERSCRIPTS_DIRECTORY));

        assert!(resolved.ends_with(Path::new("resources").join("user-agent-js")));
        assert!(resolved.is_dir());
    }

    #[test]
    fn builtin_userscripts_directory_contains_example_script() {
        let files = userscript_files(Path::new(BUILTIN_USERSCRIPTS_DIRECTORY))
            .expect("builtin userscripts directory should load");

        assert!(!files.is_empty());
        assert!(files.iter().all(|file| file.is_file()));
        assert!(files.iter().any(|file| {
            file.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "00.example.js")
        }));
    }
}
