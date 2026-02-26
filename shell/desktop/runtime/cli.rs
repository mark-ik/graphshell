/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::{env, fs, panic};
use log::warn;

use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_STARTUP_CONFIG_SNAPSHOT, CHANNEL_STARTUP_VERSE_INIT_FAILED,
    CHANNEL_STARTUP_VERSE_INIT_MODE, CHANNEL_STARTUP_VERSE_INIT_SUCCEEDED,
};

use crate::shell::desktop::host::app::App;
use crate::shell::desktop::host::event_loop::AppEventLoop;
use crate::panic_hook;
use crate::prefs::{ArgumentParsingResult, parse_command_line_arguments};

pub fn main() {
    crate::crash_handler::install();
    crate::init_crypto();
    crate::resources::init();

    // Initialize Verse mod (P2P sync capabilities) off the main thread to avoid
    // COM apartment conflicts with winit's OleInitialize path on Windows.
    // If initialization fails (e.g., keychain unavailable), log error and continue without sync.
    emit_startup_env_snapshot();
    let verse_mode = verse_init_mode();
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_STARTUP_VERSE_INIT_MODE,
        byte_len: format!("{verse_mode:?}").len(),
    });

    let verse_init_handle = match verse_mode {
        VerseInitMode::Off => None,
        VerseInitMode::Blocking => {
            if let Err(e) = crate::mods::verse::init() {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_VERSE_INIT_FAILED,
                    latency_us: 1,
                });
                log::warn!("Failed to initialize Verse mod: {}. P2P sync disabled.", e);
            } else {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_VERSE_INIT_SUCCEEDED,
                    latency_us: 1,
                });
            }
            None
        }
        VerseInitMode::Background => Some(std::thread::spawn(|| crate::mods::verse::init())),
    };

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
    maybe_enable_wsl_software_rendering_fallback(app_preferences.headless);

    let clean_shutdown = app_preferences.clean_shutdown;
    let event_loop = match app_preferences.headless {
        true => AppEventLoop::headless(),
        false => AppEventLoop::headed(),
    };

    let mut exit_code = 0;

    {
        let mut app = App::new(opts, preferences, app_preferences, &event_loop);
        if let Err(e) = event_loop.run_app(&mut app) {
            log::error!("{e}");
            exit_code = 1;
        }
    }

    if let Some(handle) = verse_init_handle {
        match handle.join() {
            Ok(Ok(())) => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_VERSE_INIT_SUCCEEDED,
                    latency_us: 1,
                });
            }
            Ok(Err(e)) => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_VERSE_INIT_FAILED,
                    latency_us: 1,
                });
                log::warn!("Failed to initialize Verse mod: {}. P2P sync disabled.", e);
            }
            Err(_) => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_VERSE_INIT_FAILED,
                    latency_us: 1,
                });
            }
        }
    }

    crate::platform::deinit(clean_shutdown);
    if exit_code != 0 {
        terminate_with_exit_code(exit_code);
    }
}

fn terminate_with_exit_code(code: i32) -> ! {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::_exit(code)
    }

    #[cfg(not(target_os = "linux"))]
    {
        std::process::exit(code)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerseInitMode {
    Off,
    Background,
    Blocking,
}

fn verse_init_mode() -> VerseInitMode {
    let Ok(value) = env::var("GRAPHSHELL_VERSE_INIT") else {
        return VerseInitMode::Background;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "0" | "false" | "disabled" => VerseInitMode::Off,
        "blocking" | "sync" | "foreground" => VerseInitMode::Blocking,
        "background" | "async" | "" => VerseInitMode::Background,
        other => {
            warn!("GRAPHSHELL_VERSE_INIT invalid ('{other}'); using background init");
            VerseInitMode::Background
        }
    }
}

fn emit_startup_env_snapshot() {
    let mut keys = Vec::new();
    for key in [
        "GRAPHSHELL_PERSISTENCE_OPEN_TIMEOUT_MS",
        "GRAPHSHELL_VERSE_INIT",
        "GRAPHSHELL_TRACING_FILTER",
        "GRAPHSHELL_GRAPH_DATA_DIR",
        "GRAPHSHELL_GRAPH_SNAPSHOT_INTERVAL_SECS",
        "GRAPHSHELL_DEVICE_PIXEL_RATIO",
        "GRAPHSHELL_SCREEN_SIZE",
        "GRAPHSHELL_WINDOW_SIZE",
        "GRAPHSHELL_HISTORY_MANAGER_LIMIT",
        "GRAPHSHELL_DISABLE_WSL_SOFTWARE_FALLBACK",
        "LIBGL_ALWAYS_SOFTWARE",
        "MESA_LOADER_DRIVER_OVERRIDE",
        "GALLIUM_DRIVER",
    ] {
        if env::var(key).is_ok() {
            keys.push(key);
        }
    }
    if keys.is_empty() {
        return;
    }
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_STARTUP_CONFIG_SNAPSHOT,
        byte_len: keys.join(",").len(),
    });
}

fn maybe_enable_wsl_software_rendering_fallback(headless: bool) {
    if headless || !running_on_wsl() || env_flag_enabled("GRAPHSHELL_DISABLE_WSL_SOFTWARE_FALLBACK") {
        return;
    }

    let mut applied = Vec::new();
    if set_env_if_unset("LIBGL_ALWAYS_SOFTWARE", "1") {
        applied.push("LIBGL_ALWAYS_SOFTWARE=1");
    }
    if set_env_if_unset("MESA_LOADER_DRIVER_OVERRIDE", "llvmpipe") {
        applied.push("MESA_LOADER_DRIVER_OVERRIDE=llvmpipe");
    }
    if set_env_if_unset("GALLIUM_DRIVER", "llvmpipe") {
        applied.push("GALLIUM_DRIVER=llvmpipe");
    }

    if !applied.is_empty() {
        warn!(
            "WSL detected: enabled software GL fallback ({}). Set GRAPHSHELL_DISABLE_WSL_SOFTWARE_FALLBACK=1 to disable.",
            applied.join(", ")
        );
    }
}

fn set_env_if_unset(key: &str, value: &str) -> bool {
    if env::var_os(key).is_some() {
        return false;
    }
    unsafe { env::set_var(key, value) };
    true
}

fn env_flag_enabled(key: &str) -> bool {
    env::var(key)
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn running_on_wsl() -> bool {
    if env::var_os("WSL_DISTRO_NAME").is_some() || env::var_os("WSL_INTEROP").is_some() {
        return true;
    }

    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            lower.contains("microsoft") || lower.contains("wsl")
        })
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn emit_startup_env_snapshot_for_tests() {
    emit_startup_env_snapshot();
}
