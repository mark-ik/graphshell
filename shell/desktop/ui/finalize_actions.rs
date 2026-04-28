/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shell-local facade for runtime finalize-action drains.
//!
//! Ungated — both the egui-host and iced-host paths call this.
//!
//! With `servo-engine`: delegates to `gui_orchestration` wrappers that
//! add diagnostic-channel instrumentation.
//! Without `servo-engine`: calls the portable runtime helpers directly
//! with a no-op clipboard-failure callback (no diagnostic infra yet on
//! the iced-only path).

use crate::app::GraphBrowserApp;
use graphshell_runtime::ports::RuntimeTickPorts;

pub(crate) fn drain_pending_runtime_finalize_actions<P>(
    graph_app: &mut GraphBrowserApp,
    ports: &mut P,
) where
    P: RuntimeTickPorts,
{
    #[cfg(feature = "servo-engine")]
    {
        crate::shell::desktop::ui::gui_orchestration::handle_pending_node_status_notices(
            graph_app, ports,
        );
        crate::shell::desktop::ui::gui_orchestration::handle_pending_clipboard_copy_requests(
            graph_app, ports,
        );
    }
    #[cfg(not(feature = "servo-engine"))]
    {
        // No webviews on the iced-only path — these queues are always empty.
        let _ = (graph_app, ports);
    }
}
