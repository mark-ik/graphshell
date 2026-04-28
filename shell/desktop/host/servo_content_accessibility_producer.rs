/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Servo-specific implementation of [`verso::ContentAccessibilityProducer`].
//!
//! `ServoContentAccessibilityProducer` wraps a shared liveness handle and
//! reports [`Active`](verso::ContentAccessibilityProducerState::Active) while
//! Servo is running, and
//! [`EngineUnavailable`](verso::ContentAccessibilityProducerState::EngineUnavailable)
//! otherwise.
//!
//! The boot path holds a [`ServoAccessibilityLiveness`] clone and calls
//! [`mark_active`](ServoAccessibilityLiveness::mark_active) once Servo is
//! initialised and [`mark_inactive`](ServoAccessibilityLiveness::mark_inactive)
//! on shutdown or crash. The host accessibility bridge state
//! (`HostBridgeUnavailable`) is not tracked in this first-pass implementation;
//! that variant is reserved for the slice that wires the platform screen-reader
//! IPC path.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use verso::{ContentAccessibilityProducer, ContentAccessibilityProducerState};

/// Shared liveness signal between the Servo boot path and
/// [`ServoContentAccessibilityProducer`].
///
/// Cheap to clone; backed by an `Arc<AtomicBool>`. The boot path holds a clone
/// and drives the state; the producer holds a clone and reads it.
#[derive(Clone, Default)]
pub(crate) struct ServoAccessibilityLiveness(Arc<AtomicBool>);

impl ServoAccessibilityLiveness {
    /// Mark the Servo engine as running. Called once Servo is successfully
    /// initialised.
    pub(crate) fn mark_active(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Mark the Servo engine as stopped. Called on clean shutdown or after a
    /// crash is detected.
    pub(crate) fn mark_inactive(&self) {
        self.0.store(false, Ordering::Release);
    }

    /// `true` while Servo is running.
    pub(crate) fn is_active(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Servo-backed [`ContentAccessibilityProducer`].
///
/// Reports engine liveness through a [`ServoAccessibilityLiveness`] handle so
/// the host accessibility bridge can degrade gracefully when Servo is absent.
pub(crate) struct ServoContentAccessibilityProducer {
    liveness: ServoAccessibilityLiveness,
}

impl ServoContentAccessibilityProducer {
    pub(crate) fn new(liveness: ServoAccessibilityLiveness) -> Self {
        Self { liveness }
    }
}

impl ContentAccessibilityProducer for ServoContentAccessibilityProducer {
    fn state(&self) -> ContentAccessibilityProducerState {
        if self.liveness.is_active() {
            ContentAccessibilityProducerState::Active
        } else {
            ContentAccessibilityProducerState::EngineUnavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_engine_unavailable() {
        let liveness = ServoAccessibilityLiveness::default();
        let producer = ServoContentAccessibilityProducer::new(liveness);
        assert_eq!(
            producer.state(),
            ContentAccessibilityProducerState::EngineUnavailable,
        );
    }

    #[test]
    fn active_after_mark_active() {
        let liveness = ServoAccessibilityLiveness::default();
        let producer = ServoContentAccessibilityProducer::new(liveness.clone());
        liveness.mark_active();
        assert_eq!(producer.state(), ContentAccessibilityProducerState::Active);
    }

    #[test]
    fn inactive_after_mark_inactive() {
        let liveness = ServoAccessibilityLiveness::default();
        let producer = ServoContentAccessibilityProducer::new(liveness.clone());
        liveness.mark_active();
        liveness.mark_inactive();
        assert_eq!(
            producer.state(),
            ContentAccessibilityProducerState::EngineUnavailable,
        );
    }

    #[test]
    fn cloned_liveness_drives_same_producer() {
        let liveness = ServoAccessibilityLiveness::default();
        let liveness2 = liveness.clone();
        let producer = ServoContentAccessibilityProducer::new(liveness);
        liveness2.mark_active();
        assert_eq!(producer.state(), ContentAccessibilityProducerState::Active);
    }
}
