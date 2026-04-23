/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host-side driver for the omnibar provider-suggestion mailbox.
//!
//! The portable state (`ProviderSuggestionMailbox` in `omnibar_state`)
//! carries an [`AsyncRequestState<T>`] with no threading primitives.
//! This module owns the concrete
//! [`BlockingTaskReceiver<ProviderSuggestionFetchOutcome>`]
//! that the portable state's [`AsyncRequestState`] is driven by.
//!
//! At each frame, [`drive_provider_suggestion_bridge`] is called before
//! the toolbar's `render_location_search_panel` inspects the mailbox.
//! It drains any pending result from the receiver and deposits it
//! into the mailbox via `state.resolve(generation, value)`. A late
//! result whose generation doesn't match the mailbox's current
//! generation is dropped as stale (the generation counter lives on
//! the mailbox; see [`ProviderSuggestionMailbox::arm_new_request`]).
//!
//! The driver itself is not portable — it holds a concrete
//! host-side blocking-task receiver. It will stay in the shell crate when the
//! `ProviderSuggestionMailbox` eventually moves to
//! `graphshell_core::shell_state::omnibar`.
//!
//! [`AsyncRequestState<T>`]: graphshell_core::async_request::AsyncRequestState
//! [`AsyncRequestState`]: graphshell_core::async_request::AsyncRequestState
//! [`ProviderSuggestionMailbox::arm_new_request`]: crate::shell::desktop::ui::omnibar_state::ProviderSuggestionMailbox::arm_new_request

use graphshell_core::async_host::{BlockingTaskReceiver, BlockingTryRecvError};
#[cfg(test)]
use graphshell_core::async_host::ErasedBlockingResult;

use crate::shell::desktop::ui::omnibar_state::{
    ProviderSuggestionFetchOutcome, ProviderSuggestionMailbox,
};

/// One pending provider-suggestion request as seen by the host.
///
/// Holds the receiver that the background worker will push its result
/// onto, tagged with the generation the mailbox was armed at. When the
/// driver bridges a frame, it passes the generation to
/// `AsyncRequestState::resolve(..)`; if the mailbox has since been
/// re-armed to a newer generation, the old result is dropped as stale.
pub(crate) struct ProviderSuggestionDriver {
    generation: u64,
    rx: BlockingTaskReceiver<ProviderSuggestionFetchOutcome>,
}

impl ProviderSuggestionDriver {
    pub(crate) fn new(
        generation: u64,
        rx: BlockingTaskReceiver<ProviderSuggestionFetchOutcome>,
    ) -> Self {
        Self { generation, rx }
    }
}

/// Frame-boundary bridge: drain any pending result from the driver's
/// receiver into the portable mailbox state. Drops the driver once
/// the request has terminated (either via a delivered result or via
/// the sender being dropped).
///
/// Call this once at the top of the toolbar frame, before any code
/// reads `mailbox.result`.
pub(crate) fn drive_provider_suggestion_bridge(
    driver_slot: &mut Option<ProviderSuggestionDriver>,
    mailbox: &mut ProviderSuggestionMailbox,
) {
    let Some(driver) = driver_slot.as_ref() else {
        return;
    };
    match driver.rx.try_recv() {
        Ok(value) => {
            let _accepted = mailbox.result.resolve(driver.generation, value);
            // Accepted or stale, the request is done — drop the driver.
            // A stale (rejected) result is fine: a newer arm has
            // already replaced the driver slot, so dropping this one
            // is the correct cleanup.
            *driver_slot = None;
        }
        Err(BlockingTryRecvError::Disconnected) => {
            // Worker went away without delivering a value (cancellation,
            // panic, ControlPanel shutdown). Mark the portable state
            // as interrupted so the consumer can synthesize a
            // user-visible failure if the request_query is still armed.
            mailbox.result.interrupt();
            *driver_slot = None;
        }
        Err(BlockingTryRecvError::Empty) => {
            // Still in flight — leave the driver and the Pending state
            // alone for another frame.
        }
        Err(BlockingTryRecvError::TypeMismatch) => {
            mailbox.result.interrupt();
            *driver_slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::desktop::ui::omnibar_state::{
        ProviderSuggestionMailbox, ProviderSuggestionStatus,
    };
    use graphshell_core::async_request::AsyncRequestState;

    fn sample_outcome() -> ProviderSuggestionFetchOutcome {
        ProviderSuggestionFetchOutcome {
            matches: Vec::new(),
            status: ProviderSuggestionStatus::Ready,
        }
    }

    #[test]
    fn bridge_delivers_result_and_retires_driver() {
        let mut mailbox = ProviderSuggestionMailbox::idle();
        let generation = mailbox.arm_new_request();
        let (tx, rx) = crossbeam_channel::bounded::<ErasedBlockingResult>(1);
        let rx = BlockingTaskReceiver::new(rx);
        let mut driver_slot = Some(ProviderSuggestionDriver::new(generation, rx));

        tx.send(Box::new(sample_outcome()) as ErasedBlockingResult)
            .expect("send sample outcome");
        drive_provider_suggestion_bridge(&mut driver_slot, &mut mailbox);

        assert!(driver_slot.is_none(), "driver should retire after delivery");
        assert!(matches!(mailbox.result, AsyncRequestState::Ready { .. }));
    }

    #[test]
    fn bridge_interrupts_when_sender_dropped_without_value() {
        let mut mailbox = ProviderSuggestionMailbox::idle();
        let generation = mailbox.arm_new_request();
        let (tx, rx) = crossbeam_channel::bounded::<ErasedBlockingResult>(1);
        let rx = BlockingTaskReceiver::new(rx);
        let mut driver_slot = Some(ProviderSuggestionDriver::new(generation, rx));

        drop(tx);
        drive_provider_suggestion_bridge(&mut driver_slot, &mut mailbox);

        assert!(driver_slot.is_none());
        assert!(matches!(mailbox.result, AsyncRequestState::Interrupted));
    }

    #[test]
    fn bridge_leaves_pending_when_receiver_is_empty() {
        let mut mailbox = ProviderSuggestionMailbox::idle();
        let generation = mailbox.arm_new_request();
        let (_tx_keep_alive, rx) = crossbeam_channel::bounded::<ErasedBlockingResult>(1);
        let rx = BlockingTaskReceiver::new(rx);
        let mut driver_slot = Some(ProviderSuggestionDriver::new(generation, rx));

        drive_provider_suggestion_bridge(&mut driver_slot, &mut mailbox);

        assert!(driver_slot.is_some(), "driver should stay in place");
        assert!(matches!(
            mailbox.result,
            AsyncRequestState::Pending { .. }
        ));
    }

    #[test]
    fn bridge_drops_stale_result_from_superseded_generation() {
        // Classic stale-late scenario: arm gen 1, arm gen 2, then
        // gen-1's rx (still held somewhere) fires. The bridge must
        // see the stale generation, refuse to deposit, and clean up.
        let mut mailbox = ProviderSuggestionMailbox::idle();
        let gen1 = mailbox.arm_new_request();
        let (tx1, rx1) = crossbeam_channel::bounded::<ErasedBlockingResult>(1);
        let rx1 = BlockingTaskReceiver::new(rx1);
        let mut driver_slot = Some(ProviderSuggestionDriver::new(gen1, rx1));

        // Caller supersedes: arm a fresh gen. The mailbox is now at
        // gen 2; driver_slot still points at the gen-1 receiver (the
        // real render-path replaces it, but this test pins what the
        // bridge itself does under a superseded generation).
        let gen2 = mailbox.arm_new_request();
        assert_ne!(gen1, gen2);

        // Now gen-1's worker finishes and pushes.
        tx1.send(Box::new(sample_outcome()) as ErasedBlockingResult)
            .expect("send gen1 result");
        drive_provider_suggestion_bridge(&mut driver_slot, &mut mailbox);

        assert!(driver_slot.is_none(), "stale driver should be dropped");
        assert!(
            matches!(mailbox.result, AsyncRequestState::Pending { generation } if generation == gen2),
            "mailbox should still be Pending at gen2 after stale result dropped"
        );
    }

    #[test]
    fn bridge_is_noop_when_driver_slot_is_empty() {
        let mut mailbox = ProviderSuggestionMailbox::idle();
        let mut driver_slot: Option<ProviderSuggestionDriver> = None;

        drive_provider_suggestion_bridge(&mut driver_slot, &mut mailbox);

        assert!(matches!(mailbox.result, AsyncRequestState::Idle));
    }
}
