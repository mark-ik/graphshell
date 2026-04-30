/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![allow(
    dead_code,
    reason = "Lane-scoped scaffolding is intentionally staged before full runtime wiring."
)]

#[cfg(all(test, feature = "servo-engine"))]
mod test;

use cfg_if::cfg_if;

// Graph browser core modules
#[path = "graph_app.rs"]
mod app;
mod domain;
mod graph;
mod input;
pub use middlenet_engine as middlenet;
mod model;
mod services;
mod shell;
mod util;

mod backtrace;
mod crash_handler;
mod mods;
mod panic_hook;
// 2026-04-25 servo-into-verso S2b: parser is Servo-typed at its
// public surface (ServoUrl). Its only consumers in graphshell main
// are themselves gated behind servo-engine (host/, lifecycle/).
#[cfg(feature = "servo-engine")]
mod parser;
// prefs is Servo-typed at most of its surface (Opts, Preferences,
// PrefValue, OutputOptions, etc.). The only host-neutral piece
// graph_app.rs depends on is `FileAccessPolicy`. When servo-engine
// is off, expose a tiny stub module with just that type so the
// app shell still compiles.
#[cfg(feature = "servo-engine")]
mod prefs;
#[cfg(not(feature = "servo-engine"))]
mod prefs {
    use std::env;
    use std::fs::read_to_string;
    use std::path::Path;
    use std::path::PathBuf;

    /// No-Servo stub of `prefs::FileAccessPolicy` for graph_app.rs.
    /// The full prefs module (Servo Opts/Preferences) only exists
    /// when servo-engine is on.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct FileAccessPolicy {
        pub allowed_directories: Vec<PathBuf>,
        pub home_directory_auto_allow: bool,
    }

    pub(crate) fn resolve_user_stylesheet_path(path: &Path) -> Result<PathBuf, std::io::Error> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(env::current_dir()?.join(path))
        }
    }

    pub(crate) fn read_user_stylesheet_source(
        path: &Path,
    ) -> Result<(PathBuf, String), std::io::Error> {
        let resolved = resolve_user_stylesheet_path(path)?;
        let source = read_to_string(&resolved)?;
        Ok((resolved, source))
    }
}
mod registries;
#[cfg(feature = "servo-engine")]
#[path = "graph_resources.rs"]
mod resources;

pub mod platform {
    #[cfg(target_os = "macos")]
    pub use crate::platform::macos::deinit;

    #[cfg(target_os = "macos")]
    pub mod macos;

    #[cfg(not(target_os = "macos"))]
    pub fn deinit(_clean_shutdown: bool) {}
}

pub fn main() {
    shell::desktop::runtime::cli::main()
}

/// Initialize Servo's bundled resource files. No-op when servo-engine
/// is off (the resource pipeline only exists for the Servo path).
pub(crate) fn init_resources() {
    #[cfg(feature = "servo-engine")]
    crate::resources::init();
}

pub fn init_crypto() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Error initializing crypto provider");
}

#[cfg(feature = "test-utils")]
pub mod test_utils {
    use std::collections::HashSet;

    pub fn active_capabilities_with_disabled(disabled_mod_ids: &[&str]) -> HashSet<String> {
        let disabled = disabled_mod_ids
            .iter()
            .map(|id| (*id).to_string())
            .collect::<HashSet<_>>();
        crate::registries::infrastructure::mod_loader::compute_active_capabilities_with_disabled(
            &disabled,
        )
    }
}

pub fn init_tracing(filter_directives: Option<&str>) {
    #[cfg(not(feature = "tracing"))]
    {
        if filter_directives.is_some() {
            log::debug!("The tracing feature was not selected - ignoring trace filter directives");
        }
    }
    #[cfg(feature = "tracing")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry();

        let subscriber =
            subscriber.with(crate::shell::desktop::runtime::tracing::PerfRingLayer::default());

        #[cfg(feature = "tracing-perfetto")]
        let subscriber = {
            // Set up a PerfettoLayer for performance tracing.
            // The graphshell.pftrace file can be uploaded to https://ui.perfetto.dev for analysis.
            let file = std::fs::File::create("graphshell.pftrace").unwrap();
            let perfetto_layer = tracing_perfetto::PerfettoLayer::new(std::sync::Mutex::new(file))
                .with_filter_by_marker(|field_name| field_name == "servo_profiling")
                .with_debug_annotations(true);
            subscriber.with(perfetto_layer)
        };

        #[cfg(feature = "tracing-hitrace")]
        let subscriber = {
            // Set up a HitraceLayer for performance tracing.
            subscriber.with(HitraceLayer::default())
        };

        // Filter events and spans by the directives in GRAPHSHELL_TRACING, using EnvFilter as a global filter.
        // <https://docs.rs/tracing-subscriber/0.3.18/tracing_subscriber/layer/index.html#global-filtering>
        let filter_builder = tracing_subscriber::EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::OFF.into());
        let filter = if let Some(filters) = &filter_directives {
            filter_builder.parse_lossy(filters)
        } else {
            filter_builder
                .with_env_var("GRAPHSHELL_TRACING")
                .from_env_lossy()
        };

        let subscriber = subscriber.with(filter);

        // Same as SubscriberInitExt::init, but avoids initialising the tracing-log compat layer,
        // since it would break Servo’s FromScriptLogger and FromEmbederLogger.
        // <https://docs.rs/tracing-subscriber/0.3.18/tracing_subscriber/util/trait.SubscriberInitExt.html#method.init>
        // <https://docs.rs/tracing/0.1.40/tracing/#consuming-log-records>
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set tracing subscriber");
    }
}

pub const VERSION: &str = concat!(
    "Graphshell ",
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("GIT_SHA")
);

/// Plumbs tracing spans into HiTrace, with the following caveats:
///
/// - We ignore spans unless they have a `servo_profiling` field.
/// - We map span entry ([`Layer::on_enter`]) to `OH_HiTrace_StartTrace(metadata.name())`.
/// - We map span exit ([`Layer::on_exit`]) to `OH_HiTrace_FinishTrace()`.
///
/// As a result, within each thread, spans must exit in reverse order of their entry, otherwise the
/// resultant profiling data will be incorrect (see the section below). This is not necessarily the
/// case for tracing spans, since there can be multiple [trace trees], so we check that this
/// invariant is upheld when debug assertions are enabled, logging errors if it is violated.
///
/// [trace trees]: https://docs.rs/tracing/0.1.40/tracing/span/index.html#span-relationships
///
/// # Uniquely identifying spans
///
/// We need to ensure that the start and end points of one span are not mixed up with other spans.
/// For now, we use the HiTrace [synchronous API], which restricts how spans must behave.
///
/// In the HiTrace [synchronous API], spans must have stack-like behaviour, because spans are keyed
/// entirely on their *name* string, and OH_HiTrace_FinishTrace always ends the most recent span.
/// While synchronous API spans are thread-local, callers could still violate this invariant with
/// reentrant or asynchronous code.
///
/// In the [asynchronous API], spans are keyed on a (*name*,*taskId*) pair, where *name* is again
/// a string, and *taskId* is an arbitrary [`i32`]. This makes *taskId* a good place for a unique
/// identifier, but asynchronous spans can cross thread boundaries, so the identifier needs to be
/// temporally unique in the whole process.
///
/// Tracing spans have such an identifier ([`Id`]), but they’re [`u64`]-based, and their format
/// is an internal implementation detail of the [`Subscriber`]. For [`Registry`], those values
/// [come from] a [packed representation] of a generation number, thread number, page number, and
/// variable-length index. This makes them hard to compress robustly into an [`i32`].
///
/// If we move to the asynchronous API, we will need to generate our own *taskId* values, perhaps
/// by combining some sort of thread id with a thread-local atomic counter. [`ThreadId`] is opaque
/// in stable Rust, and converts to a [`u64`] in unstable Rust, so we would also need to make our
/// own thread ids, perhaps by having a global atomic counter cached in a thread-local.
///
/// [synchronous API]: https://docs.rs/hitrace-sys/0.1.2/hitrace_sys/fn.OH_HiTrace_StartTrace.html
/// [asynchronous API]: https://docs.rs/hitrace-sys/0.1.2/hitrace_sys/fn.OH_HiTrace_StartAsyncTrace.html
/// [`Registry`]: tracing_subscriber::Registry
/// [come from]: https://docs.rs/tracing-subscriber/0.3.18/src/tracing_subscriber/registry/sharded.rs.html#237-269
/// [packed representation]: https://docs.rs/sharded-slab/0.1.7/sharded_slab/trait.Config.html
/// [`ThreadId`]: std::thread::ThreadId
#[cfg(feature = "tracing-hitrace")]
#[derive(Default)]
struct HitraceLayer {}

cfg_if! {
    if #[cfg(feature = "tracing-hitrace")] {
        use std::cell::RefCell;

        use tracing::span::Id;
        use tracing::Subscriber;
        use tracing_subscriber::Layer;

        #[cfg(debug_assertions)]
        thread_local! {
            /// Stack of span names, to ensure the HiTrace synchronous API is not misused.
            static HITRACE_NAME_STACK: RefCell<Vec<String>> = RefCell::default();
        }

        impl<S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>>
            Layer<S> for HitraceLayer
        {
            fn on_enter(&self, id: &Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
                if let Some(metadata) = ctx.metadata(id) {
                    // TODO: is this expensive? Would extensions be faster?
                    // <https://docs.rs/tracing-subscriber/0.3.18/tracing_subscriber/registry/struct.ExtensionsMut.html>
                    if metadata.fields().field("servo_profiling").is_some() {
                        #[cfg(debug_assertions)]
                        HITRACE_NAME_STACK.with_borrow_mut(|stack|
                            stack.push(metadata.name().to_owned()));

                        hitrace::start_trace(
                            &std::ffi::CString::new(metadata.name())
                                .expect("Failed to convert str to CString"),
                        );
                    }
                }
            }

            fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
                hitrace::start_trace(
                    &std::ffi::CString::new(event.metadata().name())
                        .expect("Failed to convert str to CString"),
                );
                hitrace::finish_trace();
            }


            fn on_exit(&self, id: &Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
                if let Some(metadata) = ctx.metadata(id) {
                    if metadata.fields().field("servo_profiling").is_some() {
                        hitrace::finish_trace();

                        #[cfg(debug_assertions)]
                        HITRACE_NAME_STACK.with_borrow_mut(|stack| {
                            if stack.last().map(|name| &**name) != Some(metadata.name()) {
                                log::error!(
                                    "Tracing span out of order: {} (stack: {:?})",
                                    metadata.name(),
                                    stack
                                );
                            }
                            if !stack.is_empty() {
                                stack.pop();
                            }
                        });
                    }
                }
            }
        }
    }
}
