/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StorageTransitionClass {
    SharedLogicalContext,
    ClonedCompatibilityContext,
    #[default]
    IsolatedFallbackContext,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BackendTransitionPlan {
    pub target_backend: String,
    pub transition_class: StorageTransitionClass,
    pub continuity_warning: Option<String>,
}
