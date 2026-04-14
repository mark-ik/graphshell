/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! NostrCore Native Mod: Host-owned Nostr capability boundary.
//!
//! This mod establishes the first-party host boundary for Nostr relay/signing
//! capabilities. It is intentionally scaffold-first: it defines and advertises
//! capability ownership and diagnostics surfaces before full relay/signing
//! runtime implementation lands.

use crate::registries::infrastructure::mod_loader::{
    ModCapability, ModManifest, ModType, NativeModRegistration,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_NOSTR_CAPABILITY_DENIED, CHANNEL_NOSTR_INTENT_REJECTED,
    CHANNEL_NOSTR_RELAY_PUBLISH_FAILED, CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
    CHANNEL_NOSTR_SECURITY_VIOLATION, CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
    register_mod_diagnostics_channel,
};

const MOD_ID: &str = "mod:nostrcore";
const MOD_OWNER_ID: &str = "nostrcore";

/// NostrCore mod manifest - registered at compile time via inventory.
pub(crate) fn nostrcore_manifest() -> ModManifest {
    ModManifest::new(
        MOD_ID,
        "NostrCore — Host Nostr Capability Boundary",
        ModType::Native,
        vec![
            "nostr:relay-subscribe".to_string(),
            "nostr:relay-publish".to_string(),
            "identity:nostr-sign".to_string(),
            "nostr:nip07-bridge".to_string(),
        ],
        vec![
            "IdentityRegistry".to_string(),
            "ActionRegistry".to_string(),
            "DiagnosticsRegistry".to_string(),
            "ControlPanel".to_string(),
        ],
        vec![ModCapability::Network, ModCapability::Identity],
    )
}

inventory::submit! {
    NativeModRegistration {
        manifest: nostrcore_manifest,
    }
}

/// NostrCore activation hook.
///
/// This registers canonical diagnostics channels for host-side Nostr policy and
/// runtime visibility. Full relay/signing service wiring is intentionally staged
/// in follow-up slices.
pub(crate) fn activate() -> Result<(), String> {
    for channel in [
        CHANNEL_NOSTR_CAPABILITY_DENIED,
        CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
        CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
        CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
        CHANNEL_NOSTR_INTENT_REJECTED,
        CHANNEL_NOSTR_SECURITY_VIOLATION,
    ] {
        if let Err(err) = register_mod_diagnostics_channel(
            MOD_OWNER_ID,
            channel,
            1,
            Some("NostrCore host-policy channel".to_string()),
        ) {
            log::warn!("nostrcore: diagnostics channel registration failed for {channel}: {err:?}");
        }
    }

    log::debug!("nostrcore: activation scaffold initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nostrcore_manifest_exposes_host_capabilities() {
        let manifest = nostrcore_manifest();
        assert_eq!(manifest.mod_id, MOD_ID);
        assert_eq!(manifest.mod_type, ModType::Native);
        assert!(
            manifest
                .provides
                .contains(&"nostr:relay-subscribe".to_string())
        );
        assert!(
            manifest
                .provides
                .contains(&"nostr:relay-publish".to_string())
        );
        assert!(
            manifest
                .provides
                .contains(&"identity:nostr-sign".to_string())
        );
        assert!(
            manifest
                .provides
                .contains(&"nostr:nip07-bridge".to_string())
        );
        assert!(manifest.capabilities.contains(&ModCapability::Network));
        assert!(manifest.capabilities.contains(&ModCapability::Identity));
    }

    #[test]
    fn nostrcore_activation_registers_namespaced_channels() {
        let result = activate();
        assert!(result.is_ok());
    }
}

