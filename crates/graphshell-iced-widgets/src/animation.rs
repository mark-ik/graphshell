/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Lightweight time-based animation primitives — one-shot
//! `Animation`s with easing, plus continuous pulse helpers for
//! ambient feedback (drag drop-zone, focus rings, recipe-derive
//! progress).
//!
//! ## Why not cosmic-time?
//!
//! The composition skeleton spec §1.5.2 mentions
//! [`cosmic-time`](https://crates.io/crates/cosmic-time) as the
//! intended keyframe engine. We're not adopting it yet because:
//!
//! - The vendored iced fork's compatibility is unverified;
//! - Most early animations are simple (open/close fades, hover
//!   transitions, ambient pulses) and don't need cosmic-time's
//!   keyframe DSL;
//! - The host already runs a 60Hz tick subscription, so pull-based
//!   animation evaluation in `view` is enough.
//!
//! When cosmic-time integration lands later, this module's surface
//! stays — call sites continue to read `progress` / `pulse`; the
//! internals swap to cosmic-time tweens.

use std::time::{Duration, Instant};

/// One-shot animation: started at `started_at`, runs over `duration`.
/// `progress(now)` returns `0.0` at start, `1.0` after `duration`,
/// clamped at both ends.
#[derive(Debug, Clone, Copy)]
pub struct Animation {
    pub started_at: Instant,
    pub duration: Duration,
}

impl Animation {
    pub fn starting_now(duration: Duration) -> Self {
        Self {
            started_at: Instant::now(),
            duration,
        }
    }

    /// Linear progress in `[0.0, 1.0]` at the given clock instant.
    pub fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now.saturating_duration_since(self.started_at);
        if elapsed >= self.duration {
            return 1.0;
        }
        elapsed.as_secs_f32() / self.duration.as_secs_f32()
    }

    pub fn finished(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }
}

// ---------------------------------------------------------------------------
// Easing curves
// ---------------------------------------------------------------------------

/// Ease-out cubic. Takes a linear `t` in `[0, 1]` and returns the
/// shaped value in `[0, 1]` — fast at the start, slow at the end.
/// Standard "feel-natural" curve for modal open / fade-in.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

/// Ease-in-out cubic. Symmetric S-curve.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let inv = -2.0 * t + 2.0;
        1.0 - inv * inv * inv / 2.0
    }
}

// ---------------------------------------------------------------------------
// Continuous pulse — for ambient feedback (drag indicator, etc.)
// ---------------------------------------------------------------------------

/// Continuous sine pulse modulating in `[0.0, 1.0]` with period
/// `period_ms`. `since` is a stable reference instant; the same
/// `since` across calls keeps the pulse phase coherent. The standard
/// pattern is to thread a single `IcedApp::startup_instant` through
/// every pulsing site so the entire UI shares one phase clock.
pub fn pulse(now: Instant, since: Instant, period_ms: u64) -> f32 {
    if period_ms == 0 {
        return 0.0;
    }
    let elapsed = now.saturating_duration_since(since).as_secs_f32() * 1000.0;
    let period = period_ms as f32;
    let phase = (elapsed / period) * std::f32::consts::TAU;
    // sin returns [-1, 1]; map to [0, 1].
    (phase.sin() + 1.0) * 0.5
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_clamps_at_both_ends() {
        let started = Instant::now();
        let anim = Animation {
            started_at: started,
            duration: Duration::from_millis(100),
        };
        // Before start (clock running backwards via saturating sub):
        assert_eq!(anim.progress(started), 0.0);
        // Mid: about 50%.
        let mid = started + Duration::from_millis(50);
        let p = anim.progress(mid);
        assert!((p - 0.5).abs() < 0.01, "got {p}");
        // After: 1.0.
        let after = started + Duration::from_millis(200);
        assert_eq!(anim.progress(after), 1.0);
    }

    #[test]
    fn zero_duration_progress_is_one() {
        let now = Instant::now();
        let anim = Animation {
            started_at: now,
            duration: Duration::from_millis(0),
        };
        assert_eq!(anim.progress(now), 1.0);
    }

    #[test]
    fn ease_out_cubic_endpoints() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
        // Mid-progress is past 50% of the curve (front-loaded).
        assert!(ease_out_cubic(0.5) > 0.5);
    }

    #[test]
    fn ease_in_out_cubic_endpoints_and_midpoint() {
        assert!((ease_in_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_in_out_cubic(1.0) - 1.0).abs() < 1e-6);
        // Symmetric: 0.5 input → 0.5 output.
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn pulse_oscillates_in_unit_range() {
        let since = Instant::now();
        for ms in (0..2000).step_by(50) {
            let now = since + Duration::from_millis(ms);
            let p = pulse(now, since, 500);
            assert!(
                (0.0..=1.0).contains(&p),
                "pulse out of [0,1] at {ms}ms: {p}",
            );
        }
    }

    #[test]
    fn pulse_with_zero_period_is_zero() {
        let since = Instant::now();
        assert_eq!(pulse(since, since, 0), 0.0);
    }
}
