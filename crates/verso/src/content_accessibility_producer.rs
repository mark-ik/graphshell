/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Accessibility producer contract for content engines.
//!
//! `ContentAccessibilityProducer` is the contract a content engine implements
//! to feed accesskit tree updates to the host's accessibility bridge.
//! `ContentAccessibilityProducerState` lets the host degrade gracefully when
//! no engine is present or the bridge is unavailable.
//!
//! Engine adapters (Servo, wry) in graphshell-main implement this trait.
//! No-Servo builds use [`AbsentContentAccessibilityProducer`], which always
//! reports `EngineUnavailable` and causes the viewer to fall back to URL +
//! title labels with no semantic tree structure.

/// Liveness state of a content engine's accessibility producer.
///
/// The host accessibility bridge (and diagnostic UI) checks this before
/// routing updates or deciding whether to show degraded viewer labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentAccessibilityProducerState {
    /// Engine is connected and feeding accesskit tree updates normally.
    Active,
    /// No content engine is present — `servo-engine` feature is off, the
    /// engine has not yet initialised, or it crashed. Viewer labels fall back
    /// to degraded mode: URL + title only, no semantic tree structure.
    EngineUnavailable,
    /// Content engine is running but the host accessibility bridge (platform
    /// screen-reader IPC, accesskit adapter) is not connected or has been
    /// dropped. Tree updates are produced but cannot be delivered to assistive
    /// technology.
    HostBridgeUnavailable,
}

/// Contract for content engines that produce accesskit tree updates.
///
/// Verso's engine adapters (Servo, wry) implement this so the host
/// accessibility bridge can poll liveness and drain pending updates without
/// hard-coding engine-specific types.
///
/// When no impl is present (no-Servo builds, engine not yet started), the host
/// uses [`AbsentContentAccessibilityProducer`], which always returns
/// `EngineUnavailable`.
pub trait ContentAccessibilityProducer {
    /// Current liveness state of the producer's engine and host bridge.
    fn state(&self) -> ContentAccessibilityProducerState;
}

/// No-op producer for builds or states where no content engine is present.
///
/// Returns [`ContentAccessibilityProducerState::EngineUnavailable`]
/// unconditionally. Used by no-Servo builds and as a placeholder before the
/// real engine-specific producer is wired in.
pub struct AbsentContentAccessibilityProducer;

impl ContentAccessibilityProducer for AbsentContentAccessibilityProducer {
    fn state(&self) -> ContentAccessibilityProducerState {
        ContentAccessibilityProducerState::EngineUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_state_variants_are_distinct() {
        assert_ne!(
            ContentAccessibilityProducerState::Active,
            ContentAccessibilityProducerState::EngineUnavailable
        );
        assert_ne!(
            ContentAccessibilityProducerState::Active,
            ContentAccessibilityProducerState::HostBridgeUnavailable
        );
        assert_ne!(
            ContentAccessibilityProducerState::EngineUnavailable,
            ContentAccessibilityProducerState::HostBridgeUnavailable
        );
    }

    #[test]
    fn absent_producer_reports_engine_unavailable() {
        let stub = AbsentContentAccessibilityProducer;
        assert_eq!(
            stub.state(),
            ContentAccessibilityProducerState::EngineUnavailable
        );
    }
}
