/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Mod activation hooks for native mods.
//!
//! When a native mod is activated by the ModRegistry, this module
//! dispatches to the appropriate mod's activation function.

use std::collections::HashMap;

/// Activation function for a native mod.
/// Returns Ok(()) on success, Err(reason) on failure.
pub(crate) type ModActivationFn = fn() -> Result<(), String>;

/// Registry of native mod activation hooks.
pub(crate) struct NativeModActivations {
    hooks: HashMap<String, ModActivationFn>,
}

impl NativeModActivations {
    /// Create a new mod activation registry with all native mod hooks.
    pub(crate) fn new() -> Self {
        let mut hooks = HashMap::new();

        // Register activation hooks for each native mod
        hooks.insert(
            "verso".to_string(),
            crate::mods::native::verso::activate as ModActivationFn,
        );
        hooks.insert(
            "verse".to_string(),
            crate::mods::native::verse::activate as ModActivationFn,
        );

        Self { hooks }
    }

    /// Activate a native mod by calling its activation function.
    pub(crate) fn activate(&self, mod_id: &str) -> Result<(), String> {
        match self.hooks.get(mod_id) {
            Some(activation_fn) => activation_fn(),
            None => {
                // Unknown mod_id means it's not a registerable native mod
                // (e.g., a WASM mod or future external mod)
                Ok(())
            }
        }
    }
}

impl Default for NativeModActivations {
    fn default() -> Self {
        Self::new()
    }
}
