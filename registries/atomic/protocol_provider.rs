/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Protocol handler provider trait for mod-based registration.
//!
//! Mods can implement this trait to register protocol handlers into the
//! ProtocolContractRegistry during activation.

use super::protocol::ProtocolContractRegistry;

/// Trait for protocol handler providers.
/// Mods implement this to register their protocol handlers during activation.
pub(crate) trait ProtocolHandlerProvider {
    /// Register this provider's protocol handlers into the registry.
    fn register(&self, registry: &mut ProtocolContractRegistry);
}

/// Global registry of protocol handler providers.
/// Used during mod activation to wire handlers into the protocol registry.
pub(crate) struct ProtocolHandlerProviders {
    providers: Vec<Box<dyn Fn(&mut ProtocolContractRegistry)>>,
}

impl ProtocolHandlerProviders {
    pub(crate) fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a protocol provider function.
    pub(crate) fn register_fn<F>(&mut self, f: F)
    where
        F: Fn(&mut ProtocolContractRegistry) + 'static,
    {
        self.providers.push(Box::new(f));
    }

    /// Apply all registered providers to the given registry.
    pub(crate) fn apply_all(&self, registry: &mut ProtocolContractRegistry) {
        for provider in &self.providers {
            provider(registry);
        }
    }
}

impl Default for ProtocolHandlerProviders {
    fn default() -> Self {
        Self::new()
    }
}
