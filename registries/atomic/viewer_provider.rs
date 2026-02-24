/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Viewer handler provider trait for mod-based registration.
//!
//! Mods can implement this trait to register viewer handlers into the
//! ViewerRegistry during activation.

use super::viewer::ViewerRegistry;

/// Trait for viewer handler providers.
/// Mods implement this to register their viewer handlers during activation.
pub(crate) trait ViewerHandlerProvider {
    /// Register this provider's viewer handlers into the registry.
    fn register(&self, registry: &mut ViewerRegistry);
}

/// Global registry of viewer handler providers.
/// Used during mod activation to wire handlers into the viewer registry.
pub(crate) struct ViewerHandlerProviders {
    providers: Vec<Box<dyn Fn(&mut ViewerRegistry)>>,
}

impl ViewerHandlerProviders {
    pub(crate) fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a viewer provider function.
    pub(crate) fn register_fn<F>(&mut self, f: F)
    where
        F: Fn(&mut ViewerRegistry) + 'static,
    {
        self.providers.push(Box::new(f));
    }

    /// Apply all registered providers to the given registry.
    pub(crate) fn apply_all(&self, registry: &mut ViewerRegistry) {
        for provider in &self.providers {
            provider(registry);
        }
    }
}

impl Default for ViewerHandlerProviders {
    fn default() -> Self {
        Self::new()
    }
}
