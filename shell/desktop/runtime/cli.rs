/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub fn main() {
    crate::crash_handler::install();
    crate::init_crypto();
    crate::init_resources();

    #[cfg(feature = "iced-host")]
    {
        let runtime = crate::shell::desktop::ui::gui_state::GraphshellRuntime::new_minimal();
        if let Err(err) = crate::shell::desktop::ui::iced_app::run_application(runtime) {
            log::error!("iced host exited with error: {err}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "iced-host"))]
    {
        log::error!("graphshell was built without the iced host");
        std::process::exit(1);
    }
}
