/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::io::Write;
use std::panic::PanicHookInfo;
use std::{env, thread};

use log::{error, warn};

pub(crate) fn panic_hook(info: &PanicHookInfo) {
    warn!("Panic hook called.");
    let msg = match info.payload().downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &**s,
            None => "Box<Any>",
        },
    };
    let current_thread = thread::current();
    let name = current_thread.name().unwrap_or("<unnamed>");
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    if let Some(location) = info.location() {
        let _ = writeln!(
            &mut stderr,
            "{} (thread {}, at {}:{})",
            msg,
            name,
            location.file(),
            location.line()
        );
    } else {
        let _ = writeln!(&mut stderr, "{} (thread {})", msg, name);
    }
    if env::var("RUST_BACKTRACE").is_ok() {
        let _ = crate::backtrace::print(&mut stderr);
    }
    drop(stderr);

    error!("{}", msg);
}
