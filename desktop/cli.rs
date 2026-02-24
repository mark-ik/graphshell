/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::{env, panic};

use crate::desktop::app::App;
use crate::desktop::event_loop::AppEventLoop;
use crate::panic_hook;
use crate::prefs::{ArgumentParsingResult, parse_command_line_arguments};

pub fn main() {
    crate::crash_handler::install();
    crate::init_crypto();
    crate::resources::init();

    // Initialize Verse mod (P2P sync capabilities)
    // If initialization fails (e.g., keychain unavailable), log error and continue without sync
    if let Err(e) = crate::mods::verse::init() {
        log::warn!("Failed to initialize Verse mod: {}. P2P sync disabled.", e);
    }

    // TODO: once log-panics is released, can this be replaced by
    // log_panics::init()?
    panic::set_hook(Box::new(panic_hook::panic_hook));

    // Skip the first argument, which is the binary name.
    let args: Vec<String> = env::args().skip(1).collect();
    let (opts, preferences, app_preferences) = match parse_command_line_arguments(&*args) {
        ArgumentParsingResult::ContentProcess(token) => return servo::run_content_process(token),
        ArgumentParsingResult::ChromeProcess(opts, preferences, app_preferences) => {
            (opts, preferences, app_preferences)
        },
        ArgumentParsingResult::Exit => {
            std::process::exit(0);
        },
        ArgumentParsingResult::ErrorParsing => {
            std::process::exit(1);
        },
    };

    crate::init_tracing(app_preferences.tracing_filter.as_deref());

    let clean_shutdown = app_preferences.clean_shutdown;
    let event_loop = match app_preferences.headless {
        true => AppEventLoop::headless(),
        false => AppEventLoop::headed(),
    };

    {
        let mut app = App::new(opts, preferences, app_preferences, &event_loop);
        event_loop.run_app(&mut app);
    }

    crate::platform::deinit(clean_shutdown)
}
