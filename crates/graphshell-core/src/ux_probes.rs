/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host-neutral UX probes — assertion-shaped consumers of the
//! [`UxEvent`](crate::ux_observability::UxEvent) stream.
//!
//! Where [`UxObserver`](crate::ux_observability::UxObserver) is a
//! passive listener (counting, recording), a `UxProbe` is an active
//! invariant-checker. Each probe encodes one rule that the iced
//! jump-ship plan §4.10 calls out — e.g., "no two modal-like
//! surfaces open at the same time" — and reports
//! [`ProbeFailure`]s when the rule is violated.
//!
//! Probes plug into the same `UxObservers` registry as plain
//! observers via [`UxProbe::as_observer`]. Tests register probes,
//! drive the iced host through a sequence of messages, and
//! `drain_failures()` returns any violations. Hosts can additionally
//! wire a probe in production to surface violations as soft warnings
//! through the diagnostics channel registry.
//!
//! ## Slice 25 ships two canonical probes
//!
//! - [`MutualExclusionProbe`] — at most one of the modal-like
//!   surfaces (Command Palette / Node Finder / Context Menu /
//!   Confirm Dialog) is open at a time. The dismissal-before-open
//!   sequencing the iced host emits during supersession satisfies
//!   this invariant; any host that opens a second modal without
//!   first dismissing the prior one trips the probe.
//! - [`OpenDismissBalanceProbe`] — every `SurfaceOpened` event is
//!   eventually paired with a matching `SurfaceDismissed`. Used to
//!   catch surface leaks where a dismissal path is forgotten.
//!
//! ## Extensibility
//!
//! Adding a probe is implementing [`UxProbe`] in any crate (no need
//! to land it in core). The two ship in core because they're
//! generally useful and have no host-specific data dependencies.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ux_observability::{SurfaceId, UxEvent, UxObserver};

/// One rule violation reported by a probe.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeFailure {
    /// Stable name of the probe that reported the failure (e.g.,
    /// `"mutual_exclusion"`). Diagnostics surfaces use this to group
    /// related failures.
    pub probe_name: &'static str,
    /// Human-readable description of the violation.
    pub description: String,
    /// The event that triggered the failure (or that immediately
    /// preceded the discovery, if the violation is detected on a
    /// later event).
    pub triggering_event: UxEvent,
}

/// A `UxProbe` observes events and accumulates failures when its
/// invariant is broken. The trait is `Send + Sync` so probes can be
/// shared across threads (iced is single-threaded today, but future
/// hosts may dispatch observers in parallel).
pub trait UxProbe: Send + Sync {
    /// Stable identifier. Used by [`ProbeFailure::probe_name`] and
    /// in diagnostics rollups.
    fn name(&self) -> &'static str;
    /// Observe the next event in the stream.
    fn observe(&self, event: &UxEvent);
    /// Drain accumulated failures since the last drain. Each call
    /// returns failures recorded since the prior call; the probe's
    /// internal failure list resets to empty.
    fn drain_failures(&self) -> Vec<ProbeFailure>;
}

/// Adapter converting a probe `Arc` into a boxed
/// [`UxObserver`](crate::ux_observability::UxObserver) suitable for
/// registration on the runtime's observer registry. The probe stays
/// queryable via the original `Arc` so the test or diagnostics-pane
/// host code can call `drain_failures()` after running messages.
pub fn probe_as_observer(probe: Arc<dyn UxProbe>) -> Box<dyn UxObserver> {
    Box::new(ProbeAdapter(probe))
}

struct ProbeAdapter(Arc<dyn UxProbe>);

impl UxObserver for ProbeAdapter {
    fn observe(&self, event: &UxEvent) {
        self.0.observe(event);
    }
}

// ---------------------------------------------------------------------------
// MutualExclusionProbe — at most one modal-like surface open at a time
// ---------------------------------------------------------------------------

/// The set of surfaces that must form a mutually-exclusive group:
/// at most one of these may be open at any given instant. Other
/// surfaces (StatusBar, NavigatorHost, panes) are always present and
/// not subject to this rule.
fn is_modal_like(surface: SurfaceId) -> bool {
    matches!(
        surface,
        SurfaceId::CommandPalette
            | SurfaceId::NodeFinder
            | SurfaceId::ContextMenu
            | SurfaceId::ConfirmDialog
    )
}

/// Asserts that at most one modal-like surface is open at a time.
/// The iced host's "dismiss-before-open" supersession sequencing
/// satisfies this — opening a second modal must emit a dismissal of
/// the prior one *first*.
pub struct MutualExclusionProbe {
    open_modals: Mutex<Vec<SurfaceId>>,
    failures: Mutex<Vec<ProbeFailure>>,
}

impl MutualExclusionProbe {
    pub fn new() -> Self {
        Self {
            open_modals: Mutex::new(Vec::new()),
            failures: Mutex::new(Vec::new()),
        }
    }
}

impl Default for MutualExclusionProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl UxProbe for MutualExclusionProbe {
    fn name(&self) -> &'static str {
        "mutual_exclusion"
    }

    fn observe(&self, event: &UxEvent) {
        match event {
            UxEvent::SurfaceOpened { surface } if is_modal_like(*surface) => {
                let mut open = self.open_modals.lock().unwrap();
                if !open.is_empty() {
                    self.failures.lock().unwrap().push(ProbeFailure {
                        probe_name: self.name(),
                        description: format!(
                            "opened {:?} while {:?} still open",
                            surface, open
                        ),
                        triggering_event: event.clone(),
                    });
                }
                open.push(*surface);
            }
            UxEvent::SurfaceDismissed { surface, .. } if is_modal_like(*surface) => {
                let mut open = self.open_modals.lock().unwrap();
                if let Some(pos) = open.iter().position(|s| s == surface) {
                    open.swap_remove(pos);
                }
            }
            _ => {}
        }
    }

    fn drain_failures(&self) -> Vec<ProbeFailure> {
        std::mem::take(&mut *self.failures.lock().unwrap())
    }
}

// ---------------------------------------------------------------------------
// OpenDismissBalanceProbe — every Opened eventually gets Dismissed
// ---------------------------------------------------------------------------

/// Asserts that every `SurfaceOpened` for a given surface gets
/// matched by a `SurfaceDismissed` before the same surface is
/// re-opened. Catches leaks where a dismissal path is forgotten.
///
/// This probe *flags on re-open*, not on stream end (the stream
/// never explicitly ends). To check terminal balance, call
/// [`Self::pending_opens`] after running messages: any non-zero
/// count means a surface is still open.
pub struct OpenDismissBalanceProbe {
    open_counts: Mutex<HashMap<SurfaceId, u32>>,
    failures: Mutex<Vec<ProbeFailure>>,
}

impl OpenDismissBalanceProbe {
    pub fn new() -> Self {
        Self {
            open_counts: Mutex::new(HashMap::new()),
            failures: Mutex::new(Vec::new()),
        }
    }

    /// Snapshot the current per-surface open count (Opens minus
    /// Dismisses). A non-zero entry means that surface is currently
    /// open — useful for terminal-balance assertions in tests.
    pub fn pending_opens(&self) -> HashMap<SurfaceId, u32> {
        self.open_counts
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, c)| **c > 0)
            .map(|(s, c)| (*s, *c))
            .collect()
    }
}

impl Default for OpenDismissBalanceProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl UxProbe for OpenDismissBalanceProbe {
    fn name(&self) -> &'static str {
        "open_dismiss_balance"
    }

    fn observe(&self, event: &UxEvent) {
        match event {
            UxEvent::SurfaceOpened { surface } => {
                let mut counts = self.open_counts.lock().unwrap();
                let entry = counts.entry(*surface).or_insert(0);
                if *entry > 0 {
                    self.failures.lock().unwrap().push(ProbeFailure {
                        probe_name: self.name(),
                        description: format!(
                            "{:?} opened again while previous open is unmatched",
                            surface
                        ),
                        triggering_event: event.clone(),
                    });
                }
                *entry += 1;
            }
            UxEvent::SurfaceDismissed { surface, .. } => {
                let mut counts = self.open_counts.lock().unwrap();
                let entry = counts.entry(*surface).or_insert(0);
                if *entry == 0 {
                    self.failures.lock().unwrap().push(ProbeFailure {
                        probe_name: self.name(),
                        description: format!(
                            "{:?} dismissed without a matching open",
                            surface
                        ),
                        triggering_event: event.clone(),
                    });
                } else {
                    *entry -= 1;
                }
            }
            _ => {}
        }
    }

    fn drain_failures(&self) -> Vec<ProbeFailure> {
        std::mem::take(&mut *self.failures.lock().unwrap())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ux_observability::{DismissReason, UxEvent, UxObservers};

    #[test]
    fn mutual_exclusion_passes_when_dismissals_precede_opens() {
        let probe = Arc::new(MutualExclusionProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        // Open palette, dismiss-superseded, open finder. The host
        // emits the dismissal *before* the new open — invariant holds.
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::CommandPalette,
        });
        observers.emit(UxEvent::SurfaceDismissed {
            surface: SurfaceId::CommandPalette,
            reason: DismissReason::Superseded,
        });
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::NodeFinder,
        });

        assert!(probe.drain_failures().is_empty());
    }

    #[test]
    fn mutual_exclusion_flags_overlapping_modals() {
        let probe = Arc::new(MutualExclusionProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        // Open palette, then open finder *without* dismissing palette.
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::CommandPalette,
        });
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::NodeFinder,
        });

        let failures = probe.drain_failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].probe_name, "mutual_exclusion");
        assert!(failures[0].description.contains("NodeFinder"));
    }

    #[test]
    fn mutual_exclusion_ignores_non_modal_surfaces() {
        let probe = Arc::new(MutualExclusionProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        // StatusBar / NavigatorHost are always-present surfaces; the
        // mutual-exclusion rule shouldn't apply to them.
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::CommandPalette,
        });
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::StatusBar,
        });

        assert!(probe.drain_failures().is_empty());
    }

    #[test]
    fn open_dismiss_balance_passes_for_paired_events() {
        let probe = Arc::new(OpenDismissBalanceProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::ContextMenu,
        });
        observers.emit(UxEvent::SurfaceDismissed {
            surface: SurfaceId::ContextMenu,
            reason: DismissReason::Cancelled,
        });

        assert!(probe.drain_failures().is_empty());
        assert!(probe.pending_opens().is_empty());
    }

    #[test]
    fn open_dismiss_balance_flags_double_open() {
        let probe = Arc::new(OpenDismissBalanceProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::ContextMenu,
        });
        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::ContextMenu,
        });

        let failures = probe.drain_failures();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].description.contains("opened again"));
    }

    #[test]
    fn open_dismiss_balance_flags_unmatched_dismiss() {
        let probe = Arc::new(OpenDismissBalanceProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        observers.emit(UxEvent::SurfaceDismissed {
            surface: SurfaceId::ContextMenu,
            reason: DismissReason::Cancelled,
        });

        let failures = probe.drain_failures();
        assert_eq!(failures.len(), 1);
        assert!(failures[0]
            .description
            .contains("dismissed without a matching open"));
    }

    #[test]
    fn open_dismiss_balance_pending_reports_unclosed_surfaces() {
        let probe = Arc::new(OpenDismissBalanceProbe::new());
        let mut observers = UxObservers::new();
        observers.register(probe_as_observer(Arc::clone(&probe) as Arc<dyn UxProbe>));

        observers.emit(UxEvent::SurfaceOpened {
            surface: SurfaceId::CommandPalette,
        });
        // Forgot the dismissal — pending_opens reports it.
        let pending = probe.pending_opens();
        assert_eq!(pending.get(&SurfaceId::CommandPalette), Some(&1));
    }
}
