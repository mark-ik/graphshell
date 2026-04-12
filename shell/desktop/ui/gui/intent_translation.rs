/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

#[cfg(test)]
pub(super) fn graph_intents_from_semantic_events(
    events: Vec<WebViewLifecycleEvent>,
) -> Vec<GraphIntent> {
    let (runtime_events, workbench_intents) =
        semantic_event_pipeline::runtime_events_from_semantic_events(events);
    debug_assert!(workbench_intents.is_empty());
    runtime_events
        .into_iter()
        .map(Into::into)
        .collect()
}

#[cfg(test)]
pub(super) fn graph_intents_and_responsive_from_events(
    events: Vec<WebViewLifecycleEvent>,
) -> (Vec<GraphIntent>, HashSet<WebViewId>) {
    let (runtime_events, workbench_intents, responsive_webviews) =
        semantic_event_pipeline::runtime_events_and_responsive_from_events(events);
    debug_assert!(workbench_intents.is_empty());
    (
        runtime_events.into_iter().map(Into::into).collect(),
        responsive_webviews,
    )
}
