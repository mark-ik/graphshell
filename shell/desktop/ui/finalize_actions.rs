/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shell-local facade for runtime finalize-action drains.
//!
//! `GraphshellRuntime` owns when finalize actions are drained, but the
//! shell still owns the current adapters that bridge `GraphBrowserApp`
//! into the extracted runtime helpers. This facade keeps that seam local
//! to `ui/` so `gui_state` no longer reaches through the broader
//! `gui_orchestration` surface just to trigger toast/clipboard drains.

use crate::app::GraphBrowserApp;
use graphshell_runtime::ports::RuntimeTickPorts;

pub(crate) fn drain_pending_runtime_finalize_actions<P>(
    graph_app: &mut GraphBrowserApp,
    ports: &mut P,
) where
    P: RuntimeTickPorts,
{
    crate::shell::desktop::ui::gui_orchestration::handle_pending_node_status_notices(
        graph_app, ports,
    );
    crate::shell::desktop::ui::gui_orchestration::handle_pending_clipboard_copy_requests(
        graph_app, ports,
    );
}
