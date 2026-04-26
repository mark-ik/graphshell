/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Omnibar session state.
//!
//! M4 slice 5b (2026-04-22) moved the portable omnibar types
//! (session, kinds, mailbox, matches, fetch outcomes) to
//! `graphshell_core::shell_state::omnibar` — `debounce_deadline`
//! switched from `std::time::Instant` to
//! `graphshell_core::time::PortableInstant`, making the entire module
//! WASM-clean. This file re-exports those types at the path callers
//! already use.
//!
//! The shell-side `ProviderSuggestionDriver` (host companion holding
//! the concrete `crossbeam_channel::Receiver`) remains in
//! `shell/desktop/ui/toolbar/toolbar_provider_driver.rs`.

#[allow(unused_imports)]
pub(crate) use graphshell_core::shell_state::omnibar::{
    HistoricalNodeMatch, OmnibarMatch, OmnibarSearchMode, OmnibarSearchSession, OmnibarSessionKind,
    ProviderSuggestionError, ProviderSuggestionFetchOutcome, ProviderSuggestionMailbox,
    ProviderSuggestionStatus, SearchProviderKind,
};
