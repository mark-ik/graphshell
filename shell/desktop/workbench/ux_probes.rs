/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_NAVIGATION_VIOLATION,
    CHANNEL_UX_STRUCTURAL_VIOLATION,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::latest_command_surface_event_sequence_metadata;

use super::ux_tree::{
    UxDomainIdentity, UxNodeRole, UxTreeSnapshot, command_surface_capture_owner_violation,
    command_surface_return_target_violation, interactive_bounds_violation,
    interactive_label_presence_violation, node_pane_tombstone_lifecycle_violation,
    presentation_id_consistency_violation, radial_sector_count_violation,
    semantic_focus_uniqueness_violation, semantic_id_uniqueness_violation,
    semantic_parent_link_violation, trace_id_consistency_violation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxContractViolation {
    pub(crate) probe_id: &'static str,
    pub(crate) channel_id: &'static str,
    pub(crate) message: String,
    pub(crate) node_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxProbeAvailability {
    Enabled,
    Disabled { reason: &'static str },
}

#[derive(Debug, Clone, Copy)]
enum UxProbeCheck {
    Stateless(fn(&UxTreeSnapshot) -> Option<UxContractViolation>),
    Stateful(fn(&UxTreeSnapshot, &mut UxProbeRuntimeState) -> Option<UxContractViolation>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UxProbeDescriptor {
    pub(crate) probe_id: &'static str,
    pub(crate) description: &'static str,
    pub(crate) availability: UxProbeAvailability,
    check: UxProbeCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UxProbeLifecycleEvent {
    Registered {
        probe_id: &'static str,
        description: &'static str,
    },
    Disabled {
        probe_id: &'static str,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxProbeRunReport {
    pub(crate) violations: Vec<UxContractViolation>,
    pub(crate) build_latency_us: u64,
    pub(crate) execution_latency_us: u64,
    pub(crate) total_latency_us: u64,
    pub(crate) registered_probe_count: usize,
    pub(crate) active_probe_count: usize,
    pub(crate) executed_probe_count: usize,
    pub(crate) skipped_for_build_budget: bool,
}

impl UxProbeRunReport {
    pub(crate) fn budget_status(&self) -> &'static str {
        if self.skipped_for_build_budget {
            "build_hard_cap_exceeded"
        } else if self.total_latency_us > UX_SEMANTICS_TOTAL_BUDGET_US {
            "total_budget_exceeded"
        } else if self.execution_latency_us > UX_PROBE_EXECUTION_BUDGET_US {
            "probe_budget_exceeded"
        } else if self.build_latency_us > UX_TREE_BUILD_INFO_BUDGET_US {
            "build_budget_exceeded"
        } else {
            "within_budget"
        }
    }
}

#[derive(Debug, Clone)]
struct UxProbeSuppressionState {
    last_emitted_at: Instant,
    suppressed_count: u64,
}

#[derive(Default)]
struct UxProbeRuntimeState {
    lifecycle_emitted: bool,
    pending_lifecycle_events: Vec<UxProbeLifecycleEvent>,
    disabled_probes: HashMap<&'static str, String>,
    suppression: HashMap<(&'static str, Option<String>), UxProbeSuppressionState>,
    placeholder_timeout_frames: HashMap<String, u64>,
    command_surface_projection_lag_frames: HashMap<&'static str, u64>,
}

static UX_PROBE_RUNTIME: OnceLock<Mutex<UxProbeRuntimeState>> = OnceLock::new();

const UX_TREE_BUILD_INFO_BUDGET_US: u64 = 500;
const UX_TREE_BUILD_HARD_CAP_US: u64 = 2_000;
const UX_PROBE_EXECUTION_BUDGET_US: u64 = 500;
const UX_SEMANTICS_TOTAL_BUDGET_US: u64 = 1_000;
const UX_PROBE_SUPPRESSION_WINDOW: Duration = Duration::from_secs(1);
const NODE_PANE_PLACEHOLDER_TIMEOUT_FRAMES: u64 = 120;
const COMMAND_SURFACE_PROJECTION_GRACE_FRAMES: u64 = 2;

fn runtime_state() -> &'static Mutex<UxProbeRuntimeState> {
    UX_PROBE_RUNTIME.get_or_init(|| Mutex::new(UxProbeRuntimeState::default()))
}

fn violation(
    probe_id: &'static str,
    channel_id: &'static str,
    message: String,
) -> UxContractViolation {
    UxContractViolation {
        probe_id,
        channel_id,
        node_path: None,
        message,
    }
}

fn violation_with_node_path(
    probe_id: &'static str,
    channel_id: &'static str,
    node_path: String,
    message: String,
) -> UxContractViolation {
    UxContractViolation {
        probe_id,
        channel_id,
        node_path: Some(node_path),
        message,
    }
}

fn check_presentation_id_consistency(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    presentation_id_consistency_violation(snapshot).map(|message| {
        violation(
            "ux.probe.presentation_id_consistency",
            CHANNEL_UX_CONTRACT_WARNING,
            message,
        )
    })
}

fn check_trace_id_consistency(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    trace_id_consistency_violation(snapshot).map(|message| {
        violation(
            "ux.probe.trace_id_consistency",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            message,
        )
    })
}

fn check_semantic_parent_links(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    semantic_parent_link_violation(snapshot).map(|message| {
        violation(
            "ux.probe.semantic_parent_links",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            message,
        )
    })
}

fn check_interactive_label_presence(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    interactive_label_presence_violation(snapshot).map(|message| {
        violation(
            "ux.probe.interactive_label_presence",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            message,
        )
    })
}

fn check_focus_uniqueness(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    semantic_focus_uniqueness_violation(snapshot).map(|message| {
        violation(
            "ux.probe.focus_uniqueness",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            message,
        )
    })
}

fn check_semantic_id_uniqueness(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    semantic_id_uniqueness_violation(snapshot).map(|message| {
        violation(
            "ux.probe.semantic_id_uniqueness",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            message,
        )
    })
}

fn check_interactive_bounds_minimum(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    interactive_bounds_violation(snapshot).map(|(node_path, message)| {
        violation_with_node_path(
            "ux.probe.interactive_bounds_minimum",
            CHANNEL_UX_CONTRACT_WARNING,
            node_path,
            message,
        )
    })
}

fn check_command_surface_capture_owner(
    snapshot: &UxTreeSnapshot,
) -> Option<UxContractViolation> {
    command_surface_capture_owner_violation(snapshot).map(|message| {
        violation(
            "ux.probe.command_surface_capture_owner",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            message,
        )
    })
}

fn check_command_surface_return_target(
    snapshot: &UxTreeSnapshot,
) -> Option<UxContractViolation> {
    command_surface_return_target_violation(snapshot).map(|message| {
        violation(
            "ux.probe.command_surface_return_target",
            CHANNEL_UX_NAVIGATION_VIOLATION,
            message,
        )
    })
}

fn check_radial_sector_count(snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
    radial_sector_count_violation(snapshot).map(|message| {
        violation(
            "ux.probe.radial_sector_count",
            CHANNEL_UX_CONTRACT_WARNING,
            message,
        )
    })
}

fn check_node_pane_tombstone_lifecycle(
    snapshot: &UxTreeSnapshot,
) -> Option<UxContractViolation> {
    node_pane_tombstone_lifecycle_violation(snapshot).map(|(node_path, message)| {
        violation_with_node_path(
            "ux.probe.node_pane_tombstone_lifecycle",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            node_path,
            message,
        )
    })
}

fn check_node_pane_placeholder_timeout(
    snapshot: &UxTreeSnapshot,
    state: &mut UxProbeRuntimeState,
) -> Option<UxContractViolation> {
    let mut observed_candidates = HashSet::new();
    let mut violation = None;

    for node in &snapshot.semantic_nodes {
        if node.role != UxNodeRole::NodePane || !node.state.degraded {
            continue;
        }

        let UxDomainIdentity::Node {
            node_key,
            attach_attempt: Some(attach_attempt),
            ..
        } = &node.domain
        else {
            continue;
        };

        if attach_attempt.retry_count == 0
            && attach_attempt.pending_attempt_age_ms.is_none()
            && attach_attempt.cooldown_remaining_ms.is_none()
        {
            continue;
        }

        observed_candidates.insert(node.ux_node_id.clone());
        let frames = state
            .placeholder_timeout_frames
            .entry(node.ux_node_id.clone())
            .or_insert(0);
        *frames = frames.saturating_add(1);

        if *frames > NODE_PANE_PLACEHOLDER_TIMEOUT_FRAMES && violation.is_none() {
            violation = Some(violation_with_node_path(
                "ux.probe.node_pane_placeholder_timeout",
                CHANNEL_UX_CONTRACT_WARNING,
                node.ux_node_id.clone(),
                format!(
                    "uxtree invariant failed: NodePane '{}' for node {:?} stayed degraded for {} consecutive frames after attach attempts (retries={}, pending_age_ms={:?}, cooldown_remaining_ms={:?})",
                    node.ux_node_id,
                    node_key,
                    frames,
                    attach_attempt.retry_count,
                    attach_attempt.pending_attempt_age_ms,
                    attach_attempt.cooldown_remaining_ms,
                ),
            ));
        }
    }

    state
        .placeholder_timeout_frames
        .retain(|ux_node_id, _| observed_candidates.contains(ux_node_id));
    violation
}

fn projected_command_bar_route_events(
    snapshot: &UxTreeSnapshot,
) -> Option<crate::shell::desktop::ui::toolbar::toolbar_ui::CommandRouteEventSequenceMetadata> {
    snapshot.semantic_nodes.iter().find_map(|node| {
        if node.role != UxNodeRole::CommandBar {
            return None;
        }

        match &node.domain {
            UxDomainIdentity::CommandBar { route_events, .. } => Some(*route_events),
            _ => None,
        }
    })
}

fn projected_omnibar_mailbox_events(
    snapshot: &UxTreeSnapshot,
) -> Option<crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarMailboxEventSequenceMetadata> {
    snapshot.semantic_nodes.iter().find_map(|node| {
        if node.role != UxNodeRole::Omnibar {
            return None;
        }

        match &node.domain {
            UxDomainIdentity::Omnibar { mailbox_events, .. } => Some(*mailbox_events),
            _ => None,
        }
    })
}

fn lagging_projection_frames(
    state: &mut UxProbeRuntimeState,
    key: &'static str,
    is_lagging: bool,
) -> u64 {
    if !is_lagging {
        state.command_surface_projection_lag_frames.remove(key);
        return 0;
    }

    let frames = state.command_surface_projection_lag_frames.entry(key).or_insert(0);
    *frames = frames.saturating_add(1);
    *frames
}

fn check_command_surface_observability_projection(
    snapshot: &UxTreeSnapshot,
    state: &mut UxProbeRuntimeState,
) -> Option<UxContractViolation> {
    let live = latest_command_surface_event_sequence_metadata();
    let projected_routes = projected_command_bar_route_events(snapshot).unwrap_or_default();
    let projected_mailbox = projected_omnibar_mailbox_events(snapshot).unwrap_or_default();

    let stale_lag_frames = lagging_projection_frames(
        state,
        "omnibar_mailbox_stale",
        live.omnibar_mailbox_events.stale > projected_mailbox.stale,
    );
    if stale_lag_frames >= COMMAND_SURFACE_PROJECTION_GRACE_FRAMES {
        return Some(violation(
            "ux.probe.command_surface_observability_projection",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            format!(
                "uxtree invariant failed: omnibar stale-mailbox observability was dropped from the semantic snapshot (live stale_seq={}, projected stale_seq={})",
                live.omnibar_mailbox_events.stale,
                projected_mailbox.stale,
            ),
        ));
    }

    let no_target_lag_frames = lagging_projection_frames(
        state,
        "command_route_no_target",
        live.route_events.no_target > projected_routes.no_target,
    );
    if no_target_lag_frames >= COMMAND_SURFACE_PROJECTION_GRACE_FRAMES {
        return Some(violation(
            "ux.probe.command_surface_observability_projection",
            CHANNEL_UX_STRUCTURAL_VIOLATION,
            format!(
                "uxtree invariant failed: command-route no-target observability was dropped from the semantic snapshot (live no_target_seq={}, projected no_target_seq={})",
                live.route_events.no_target,
                projected_routes.no_target,
            ),
        ));
    }

    None
}

static CORE_UX_PROBES: [UxProbeDescriptor; 13] = [
    UxProbeDescriptor {
        probe_id: "ux.probe.presentation_id_consistency",
        description: "Presentation-layer ux_node_id values must exist in the semantic layer.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_presentation_id_consistency),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.trace_id_consistency",
        description: "Trace-layer ux_node_id values must exist in the semantic layer.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_trace_id_consistency),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.semantic_parent_links",
        description: "Semantic nodes must reference existing semantic parents.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_semantic_parent_links),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.interactive_label_presence",
        description: "Interactive semantic nodes must have a non-empty label.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_interactive_label_presence),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.focus_uniqueness",
        description: "At most one semantic node may advertise focus in a single snapshot.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_focus_uniqueness),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.semantic_id_uniqueness",
        description: "Semantic ux_node_id values must be unique within a single snapshot.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_semantic_id_uniqueness),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.interactive_bounds_minimum",
        description: "Interactive semantic nodes with bounds must be at least 32x32 logical pixels.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_interactive_bounds_minimum),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.command_surface_capture_owner",
        description: "Only one command-surface capture owner may advertise semantic focus.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_command_surface_capture_owner),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.command_surface_return_target",
        description: "Visible command surfaces must advertise a return target or fallback anchor.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_command_surface_return_target),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.radial_sector_count",
        description: "Radial palette snapshots must project between one and eight radial sectors when present.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_radial_sector_count),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.node_pane_tombstone_lifecycle",
        description: "Visible NodePane semantic nodes must not project tombstoned graph-node lifecycle state.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateless(check_node_pane_tombstone_lifecycle),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.node_pane_placeholder_timeout",
        description: "Degraded NodePane semantic nodes must not remain in placeholder mode for more than 120 consecutive frames after attach attempts.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateful(check_node_pane_placeholder_timeout),
    },
    UxProbeDescriptor {
        probe_id: "ux.probe.command_surface_observability_projection",
        description: "Command-surface stale-mailbox and no-target route diagnostics must remain observable in semantic snapshots.",
        availability: UxProbeAvailability::Enabled,
        check: UxProbeCheck::Stateful(check_command_surface_observability_projection),
    },
];

pub(crate) fn registered_probes() -> &'static [UxProbeDescriptor] {
    &CORE_UX_PROBES
}

fn lifecycle_events_for_descriptors(
    descriptors: &[UxProbeDescriptor],
) -> Vec<UxProbeLifecycleEvent> {
    descriptors
        .iter()
        .map(|descriptor| match descriptor.availability {
            UxProbeAvailability::Enabled => UxProbeLifecycleEvent::Registered {
                probe_id: descriptor.probe_id,
                description: descriptor.description,
            },
            UxProbeAvailability::Disabled { reason } => UxProbeLifecycleEvent::Disabled {
                probe_id: descriptor.probe_id,
                reason: reason.to_string(),
            },
        })
        .collect()
}

pub(crate) const fn runtime_enabled() -> bool {
    cfg!(feature = "ux-probes")
}

pub(crate) fn drain_probe_lifecycle_events() -> Vec<UxProbeLifecycleEvent> {
    if !runtime_enabled() {
        return Vec::new();
    }
    let Ok(mut state) = runtime_state().lock() else {
        return Vec::new();
    };
    let mut events = if state.lifecycle_emitted {
        Vec::new()
    } else {
        state.lifecycle_emitted = true;
        lifecycle_events_for_descriptors(registered_probes())
    };
    if !state.pending_lifecycle_events.is_empty() {
        events.append(&mut state.pending_lifecycle_events);
    }
    events
}

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn runtime_disabled_probe_ids() -> HashMap<&'static str, String> {
    runtime_state()
        .lock()
        .map(|state| state.disabled_probes.clone())
        .unwrap_or_default()
}

fn disable_probe_for_session(probe_id: &'static str, reason: String) {
    let Ok(mut state) = runtime_state().lock() else {
        return;
    };
    if state.disabled_probes.contains_key(probe_id) {
        return;
    }
    state.disabled_probes.insert(probe_id, reason.clone());
    state
        .pending_lifecycle_events
        .push(UxProbeLifecycleEvent::Disabled { probe_id, reason });
}

fn suppress_violation(
    mut violation: UxContractViolation,
    now: Instant,
) -> Option<UxContractViolation> {
    let Ok(mut state) = runtime_state().lock() else {
        return Some(violation);
    };
    let key = (violation.probe_id, violation.node_path.clone());
    if let Some(entry) = state.suppression.get_mut(&key) {
        if now.duration_since(entry.last_emitted_at) < UX_PROBE_SUPPRESSION_WINDOW {
            entry.suppressed_count = entry.suppressed_count.saturating_add(1);
            return None;
        }
        let suppressed_count = entry.suppressed_count;
        entry.last_emitted_at = now;
        entry.suppressed_count = 0;
        if suppressed_count > 0 {
            violation.message = format!(
                "{} (suppressed {} repeats)",
                violation.message, suppressed_count
            );
        }
        return Some(violation);
    }
    state.suppression.insert(
        key,
        UxProbeSuppressionState {
            last_emitted_at: now,
            suppressed_count: 0,
        },
    );
    Some(violation)
}

fn evaluate_descriptors(
    snapshot: &UxTreeSnapshot,
    descriptors: &[UxProbeDescriptor],
    build_latency_us: u64,
) -> UxProbeRunReport {
    let registered_probe_count = descriptors.len();
    let runtime_disabled = runtime_disabled_probe_ids();
    let active_probe_count = descriptors
        .iter()
        .filter(|descriptor| {
            matches!(descriptor.availability, UxProbeAvailability::Enabled)
                && !runtime_disabled.contains_key(descriptor.probe_id)
        })
        .count();
    if build_latency_us > UX_TREE_BUILD_HARD_CAP_US {
        return UxProbeRunReport {
            violations: Vec::new(),
            build_latency_us,
            execution_latency_us: 0,
            total_latency_us: build_latency_us,
            registered_probe_count,
            active_probe_count,
            executed_probe_count: 0,
            skipped_for_build_budget: true,
        };
    }

    let started_at = Instant::now();
    let mut violations = Vec::new();
    let mut executed_probe_count = 0;

    for descriptor in descriptors {
        if !matches!(descriptor.availability, UxProbeAvailability::Enabled) {
            continue;
        }
        if runtime_disabled_probe_ids().contains_key(descriptor.probe_id) {
            continue;
        }
        executed_probe_count += 1;
        let now = Instant::now();
        let evaluation = match descriptor.check {
            UxProbeCheck::Stateless(check) => catch_unwind(AssertUnwindSafe(|| check(snapshot))),
            UxProbeCheck::Stateful(check) => catch_unwind(AssertUnwindSafe(|| {
                let Ok(mut state) = runtime_state().lock() else {
                    return None;
                };
                check(snapshot, &mut state)
            })),
        };
        match evaluation {
            Ok(Some(violation)) => {
                if let Some(violation) = suppress_violation(violation, now) {
                    violations.push(violation);
                }
            }
            Ok(None) => {}
            Err(payload) => {
                let panic_message = panic_payload_message(payload);
                disable_probe_for_session(
                    descriptor.probe_id,
                    format!("probe panicked: {}", panic_message),
                );
                violations.push(violation(
                    descriptor.probe_id,
                    CHANNEL_UX_CONTRACT_WARNING,
                    format!(
                        "UxProbe {} panicked: {}",
                        descriptor.probe_id, panic_message
                    ),
                ));
            }
        }
    }

    let execution_latency_us = started_at.elapsed().as_micros() as u64;
    UxProbeRunReport {
        violations,
        build_latency_us,
        execution_latency_us,
        total_latency_us: build_latency_us.saturating_add(execution_latency_us),
        registered_probe_count,
        active_probe_count,
        executed_probe_count,
        skipped_for_build_budget: false,
    }
}

pub(crate) fn evaluate_registered_probes(
    snapshot: &UxTreeSnapshot,
    build_latency_us: u64,
) -> UxProbeRunReport {
    if !runtime_enabled() {
        return UxProbeRunReport {
            violations: Vec::new(),
            build_latency_us,
            execution_latency_us: 0,
            total_latency_us: build_latency_us,
            registered_probe_count: registered_probes().len(),
            active_probe_count: 0,
            executed_probe_count: 0,
            skipped_for_build_budget: false,
        };
    }
    evaluate_descriptors(snapshot, registered_probes(), build_latency_us)
}

#[cfg(test)]
pub(crate) fn reset_probe_runtime_for_tests() {
    if let Ok(mut state) = runtime_state().lock() {
        *state = UxProbeRuntimeState::default();
    }
}

#[cfg(test)]
fn age_suppression_window_for_tests(probe_id: &'static str, node_path: Option<&str>) {
    if let Ok(mut state) = runtime_state().lock() {
        if let Some(entry) = state
            .suppression
            .get_mut(&(probe_id, node_path.map(str::to_string)))
        {
            entry.last_emitted_at = Instant::now() - UX_PROBE_SUPPRESSION_WINDOW;
        }
    }
}

#[cfg(test)]
fn drain_pending_probe_lifecycle_events_for_tests() -> Vec<UxProbeLifecycleEvent> {
    let Ok(mut state) = runtime_state().lock() else {
        return Vec::new();
    };
    std::mem::take(&mut state.pending_lifecycle_events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    use crate::render::radial_menu::{
        RadialPaletteSemanticSnapshot, RadialPaletteSemanticSummary, RadialSectorSemanticMetadata,
        clear_semantic_snapshot, publish_semantic_snapshot,
    };
    use crate::shell::desktop::lifecycle::webview_backpressure::NodePaneAttachAttemptMetadata;
    use crate::shell::desktop::tests::harness::TestRegistry;
    use crate::shell::desktop::ui::toolbar::toolbar_ui::{
        CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
        CommandSurfaceEventSequenceMetadata, CommandSurfaceSemanticSnapshot,
        OmnibarMailboxEventSequenceMetadata, OmnibarSemanticMetadata,
        PaletteSurfaceSemanticMetadata, clear_command_surface_semantic_snapshot,
        lock_command_surface_snapshot_tests, publish_command_surface_semantic_snapshot,
        set_command_surface_event_sequence_metadata_for_tests,
    };

    fn lock_probe_tests() -> std::sync::MutexGuard<'static, ()> {
        static UX_PROBE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        UX_PROBE_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("probe test lock should not be poisoned")
    }

    fn test_violation(_snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
        Some(UxContractViolation {
            probe_id: "ux.probe.test",
            channel_id: CHANNEL_UX_CONTRACT_WARNING,
            message: "synthetic violation".to_string(),
            node_path: Some("uxnode://test/node".to_string()),
        })
    }

    #[test]
    fn drain_probe_lifecycle_events_emits_core_registration_once() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let first = drain_probe_lifecycle_events();
        let second = drain_probe_lifecycle_events();

        assert_eq!(first.len(), registered_probes().len());
        assert!(first.iter().all(|event| matches!(
            event,
            UxProbeLifecycleEvent::Registered { .. }
        )));
        assert!(second.is_empty());
    }

    #[test]
    fn lifecycle_events_include_disabled_descriptors() {
        let _guard = lock_probe_tests();
        let descriptors = [
            UxProbeDescriptor {
                probe_id: "ux.probe.enabled",
                description: "enabled probe",
                availability: UxProbeAvailability::Enabled,
                check: UxProbeCheck::Stateless(test_violation),
            },
            UxProbeDescriptor {
                probe_id: "ux.probe.disabled",
                description: "disabled probe",
                availability: UxProbeAvailability::Disabled {
                    reason: "feature inactive",
                },
                check: UxProbeCheck::Stateless(test_violation),
            },
        ];

        let events = lifecycle_events_for_descriptors(&descriptors);

        assert!(events.iter().any(|event| matches!(
            event,
            UxProbeLifecycleEvent::Registered { probe_id, .. } if *probe_id == "ux.probe.enabled"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            UxProbeLifecycleEvent::Disabled { probe_id, reason }
                if *probe_id == "ux.probe.disabled" && reason == "feature inactive"
        )));
    }

    fn panicking_probe(_snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
        panic!("synthetic panic")
    }

    fn healthy_probe_after_panic(_snapshot: &UxTreeSnapshot) -> Option<UxContractViolation> {
        Some(UxContractViolation {
            probe_id: "ux.probe.still_runs",
            channel_id: CHANNEL_UX_CONTRACT_WARNING,
            message: "healthy probe still ran".to_string(),
            node_path: Some("uxnode://test/healthy".to_string()),
        })
    }

    fn snapshot_for_probe_tests() -> UxTreeSnapshot {
        let harness = TestRegistry::new();
        crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            3,
        )
    }

    #[test]
    fn panicking_probe_is_disabled_without_stopping_other_probes() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();
        let snapshot = snapshot_for_probe_tests();
        let descriptors = [
            UxProbeDescriptor {
                probe_id: "ux.probe.panics",
                description: "panicking probe",
                availability: UxProbeAvailability::Enabled,
                check: UxProbeCheck::Stateless(panicking_probe),
            },
            UxProbeDescriptor {
                probe_id: "ux.probe.still_runs",
                description: "healthy probe",
                availability: UxProbeAvailability::Enabled,
                check: UxProbeCheck::Stateless(healthy_probe_after_panic),
            },
        ];

        let first = evaluate_descriptors(&snapshot, &descriptors, 0);
        let lifecycle = drain_pending_probe_lifecycle_events_for_tests();
        let second = evaluate_descriptors(&snapshot, &descriptors, 0);

        assert_eq!(first.executed_probe_count, 2);
        assert!(first.violations.iter().any(|violation| {
            violation.probe_id == "ux.probe.panics"
                && violation.channel_id == CHANNEL_UX_CONTRACT_WARNING
                && violation.message.contains("panicked: synthetic panic")
        }));
        assert!(first.violations.iter().any(|violation| {
            violation.probe_id == "ux.probe.still_runs"
                && violation.message == "healthy probe still ran"
        }));
        assert!(lifecycle.iter().any(|event| matches!(
            event,
            UxProbeLifecycleEvent::Disabled { probe_id, reason }
                if *probe_id == "ux.probe.panics" && reason.contains("synthetic panic")
        )));
        assert_eq!(second.executed_probe_count, 1);
        assert!(!second.violations.iter().any(|violation| {
            violation.probe_id == "ux.probe.panics"
        }));
    }

    #[test]
    fn repeated_violation_is_suppressed_until_window_elapses() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();
        let snapshot = snapshot_for_probe_tests();
        let descriptors = [UxProbeDescriptor {
            probe_id: "ux.probe.test",
            description: "suppressed probe",
            availability: UxProbeAvailability::Enabled,
            check: UxProbeCheck::Stateless(test_violation),
        }];

        let first = evaluate_descriptors(&snapshot, &descriptors, 0);
        let second = evaluate_descriptors(&snapshot, &descriptors, 0);
        age_suppression_window_for_tests("ux.probe.test", Some("uxnode://test/node"));
        let third = evaluate_descriptors(&snapshot, &descriptors, 0);

        assert_eq!(first.violations.len(), 1);
        assert!(second.violations.is_empty());
        assert_eq!(third.violations.len(), 1);
        assert!(third.violations[0]
            .message
            .contains("suppressed 1 repeats"));
    }

    #[test]
    fn build_budget_skip_prevents_probe_execution_for_frame() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();
        let snapshot = snapshot_for_probe_tests();
        let descriptors = [UxProbeDescriptor {
            probe_id: "ux.probe.test",
            description: "budgeted probe",
            availability: UxProbeAvailability::Enabled,
            check: UxProbeCheck::Stateless(test_violation),
        }];

        let report = evaluate_descriptors(&snapshot, &descriptors, UX_TREE_BUILD_HARD_CAP_US + 1);

        assert!(report.skipped_for_build_budget);
        assert_eq!(report.executed_probe_count, 0);
        assert!(report.violations.is_empty());
        assert_eq!(report.budget_status(), "build_hard_cap_exceeded");
    }

    #[test]
    fn evaluate_registered_probes_surfaces_presentation_id_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-probes.example");
        harness.open_node_tab(node);
        let mut snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            5,
        );
        snapshot
            .presentation_nodes
            .retain(|node| node.ux_node_id != crate::shell::desktop::workbench::ux_tree::UX_TREE_WORKBENCH_ROOT_ID);
        snapshot.presentation_nodes.push(
            crate::shell::desktop::workbench::ux_tree::UxPresentationNode {
                ux_node_id: "uxnode://orphan/presentation".to_string(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.orphan",
                style_flags: Vec::new(),
                transient_flags: Vec::new(),
            },
        );

        let report = evaluate_registered_probes(&snapshot, 0);

        assert!(report.violations.iter().any(|violation| {
            violation.probe_id == "ux.probe.presentation_id_consistency"
                && violation.channel_id == CHANNEL_UX_CONTRACT_WARNING
                && violation.message.contains("orphan/presentation")
        }));
    }

    #[test]
    fn evaluate_registered_probes_surfaces_navigation_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let _guard = lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: false,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: false,
                focused: false,
                query: None,
                match_count: 0,
                provider_status: None,
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata {
                contextual_mode: false,
                return_target: None,
                pending_node_context_target: None,
                pending_frame_context_target: None,
                context_anchor_present: false,
            }),
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            7,
        );

        assert!(crate::shell::desktop::workbench::ux_tree::command_surface_return_target_violation(&snapshot).is_some());

        let report = evaluate_registered_probes(&snapshot, 0);

        assert!(report.violations.iter().any(|violation| {
            violation.probe_id == "ux.probe.command_surface_return_target"
                && violation.channel_id == CHANNEL_UX_NAVIGATION_VIOLATION
                && violation.message.contains("command palette")
        }));

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn evaluate_registered_probes_surfaces_interactive_bounds_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-probes-bounds.example");
        harness.open_node_tab(node);
        let mut snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            9,
        );

        let interactive_id = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                !entry.allowed_actions.is_empty()
                    && snapshot
                        .presentation_nodes
                        .iter()
                        .any(|presentation| presentation.ux_node_id == entry.ux_node_id)
            })
            .expect("snapshot should contain an interactive node with presentation")
            .ux_node_id
            .clone();
        let presentation = snapshot
            .presentation_nodes
            .iter_mut()
            .find(|entry| entry.ux_node_id == interactive_id)
            .expect("interactive node should have presentation metadata");
        presentation.bounds = Some([0.0, 0.0, 24.0, 18.0]);

        let report = evaluate_registered_probes(&snapshot, 0);

        assert!(report.violations.iter().any(|violation| {
            violation.probe_id == "ux.probe.interactive_bounds_minimum"
                && violation.channel_id == CHANNEL_UX_CONTRACT_WARNING
                && violation.node_path.as_deref() == Some(interactive_id.as_str())
                && violation.message.contains("24.0x18.0")
        }));
    }

    #[test]
    fn evaluate_registered_probes_surfaces_focus_uniqueness_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-probes-focus.example");
        harness.open_node_tab(node);
        let mut snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            9,
        );

        let mut focused_nodes = snapshot
            .semantic_nodes
            .iter_mut()
            .filter(|entry| !entry.allowed_actions.is_empty())
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(focused_nodes.len(), 2, "expected two interactive nodes for focus collision test");
        focused_nodes[0].state.focused = true;
        focused_nodes[1].state.focused = true;

        assert!(crate::shell::desktop::workbench::ux_tree::semantic_focus_uniqueness_violation(&snapshot).is_some());

        let report = evaluate_registered_probes(&snapshot, 0);

        assert!(
            report.violations.iter().any(|violation| {
                violation.probe_id == "ux.probe.focus_uniqueness"
                    && violation.channel_id == CHANNEL_UX_STRUCTURAL_VIOLATION
                    && violation.message.contains("multiple focused semantic nodes")
            }),
            "expected focus uniqueness violation, got {:?}",
            report.violations
        );
    }

    #[test]
    fn evaluate_registered_probes_surfaces_tombstoned_node_pane_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-probes-tombstone-pane.example");
        harness.open_node_tab(node);
        let mut snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            9,
        );

        let node_pane_id = {
            let node_pane = snapshot
                .semantic_nodes
                .iter_mut()
                .find(|entry| {
                    entry.role == crate::shell::desktop::workbench::ux_tree::UxNodeRole::NodePane
                })
                .expect("snapshot should contain a node pane semantic node");
            node_pane.domain = crate::shell::desktop::workbench::ux_tree::UxDomainIdentity::Node {
                node_key: node,
                pane_id: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                lifecycle: crate::graph::NodeLifecycle::Tombstone,
                attach_attempt: None,
            };
            node_pane.ux_node_id.clone()
        };

        assert!(crate::shell::desktop::workbench::ux_tree::node_pane_tombstone_lifecycle_violation(&snapshot).is_some());

        let report = evaluate_registered_probes(&snapshot, 0);

        assert!(
            report.violations.iter().any(|violation| {
                violation.probe_id == "ux.probe.node_pane_tombstone_lifecycle"
                    && violation.channel_id == CHANNEL_UX_STRUCTURAL_VIOLATION
                    && violation.node_path.as_deref() == Some(node_pane_id.as_str())
                    && violation.message.contains("tombstoned node")
            }),
            "expected tombstoned node pane violation, got {:?}",
            report.violations
        );
    }

    #[test]
    fn evaluate_registered_probes_surfaces_radial_sector_count_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();
        clear_semantic_snapshot();

        publish_semantic_snapshot(RadialPaletteSemanticSnapshot {
            sectors: (0..9)
                .map(|index| RadialSectorSemanticMetadata {
                    tier: 1,
                    domain_label: format!("Domain {index}"),
                    action_id: format!("action.{index}"),
                    enabled: true,
                    page: 0,
                    rail_position: index as f32 / 9.0,
                    angle_rad: index as f32,
                    hover_scale: 1.0,
                })
                .collect(),
            summary: RadialPaletteSemanticSummary {
                tier1_visible_count: 9,
                tier2_visible_count: 0,
                tier2_page: 0,
                tier2_page_count: 0,
                overflow_hidden_entries: 1,
                label_pre_collisions: 0,
                label_post_collisions: 0,
                fallback_to_palette: false,
                fallback_reason: None,
            },
        });

        let harness = TestRegistry::new();
        let snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            14,
        );
        let report = evaluate_registered_probes(&snapshot, 0);
        clear_semantic_snapshot();

        assert!(
            report.violations.iter().any(|violation| {
                violation.probe_id == "ux.probe.radial_sector_count"
                    && violation.channel_id == CHANNEL_UX_CONTRACT_WARNING
                    && violation.message.contains("9 radial sectors")
            }),
            "expected radial sector count violation, got {:?}",
            report.violations
        );
    }

    #[test]
    fn evaluate_registered_probes_surfaces_placeholder_timeout_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-probes-placeholder.example");
        harness.open_node_tab(node);
        let mut snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            11,
        );

        let node_pane_id = {
            let node_pane = snapshot
                .semantic_nodes
                .iter_mut()
                .find(|entry| {
                    entry.role == crate::shell::desktop::workbench::ux_tree::UxNodeRole::NodePane
                })
                .expect("snapshot should contain a node pane semantic node");
            node_pane.state.degraded = true;
            node_pane.domain = crate::shell::desktop::workbench::ux_tree::UxDomainIdentity::Node {
                node_key: node,
                pane_id: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                lifecycle: crate::graph::NodeLifecycle::Active,
                attach_attempt: Some(NodePaneAttachAttemptMetadata {
                    retry_count: 3,
                    pending_attempt_age_ms: Some(8_250),
                    cooldown_remaining_ms: None,
                }),
            };
            node_pane.ux_node_id.clone()
        };

        let mut report = UxProbeRunReport {
            violations: Vec::new(),
            build_latency_us: 0,
            execution_latency_us: 0,
            total_latency_us: 0,
            registered_probe_count: 0,
            active_probe_count: 0,
            executed_probe_count: 0,
            skipped_for_build_budget: false,
        };
        for _ in 0..=NODE_PANE_PLACEHOLDER_TIMEOUT_FRAMES {
            report = evaluate_registered_probes(&snapshot, 0);
        }

        assert!(
            report.violations.iter().any(|violation| {
                violation.probe_id == "ux.probe.node_pane_placeholder_timeout"
                    && violation.channel_id == CHANNEL_UX_CONTRACT_WARNING
                    && violation.node_path.as_deref() == Some(node_pane_id.as_str())
                    && violation.message.contains("consecutive frames after attach attempts")
            }),
            "expected placeholder timeout violation, got {:?}",
            report.violations
        );
    }

    #[test]
    fn evaluate_registered_probes_surfaces_command_surface_observability_violation() {
        let _guard = lock_probe_tests();
        reset_probe_runtime_for_tests();

        let _guard = lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        set_command_surface_event_sequence_metadata_for_tests(CommandSurfaceEventSequenceMetadata {
            route_events: CommandRouteEventSequenceMetadata {
                resolved: 0,
                fallback: 0,
                no_target: 2,
            },
            omnibar_mailbox_events: OmnibarMailboxEventSequenceMetadata {
                request_started: 1,
                applied: 0,
                failed: 0,
                stale: 1,
            },
        });
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: false,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: false,
                focused: false,
                query: None,
                match_count: 0,
                provider_status: None,
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: None,
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = crate::shell::desktop::workbench::ux_tree::build_snapshot(
            &harness.tiles_tree,
            &harness.app,
            13,
        );

        let first = evaluate_registered_probes(&snapshot, 0);
        age_suppression_window_for_tests(
            "ux.probe.command_surface_observability_projection",
            None,
        );
        let second = evaluate_registered_probes(&snapshot, 0);

        assert!(
            first
                .violations
                .iter()
                .chain(second.violations.iter())
                .any(|violation| {
                violation.probe_id == "ux.probe.command_surface_observability_projection"
                    && violation.channel_id == CHANNEL_UX_STRUCTURAL_VIOLATION
                    && violation.message.contains("stale-mailbox observability")
            }),
            "expected command-surface observability violation, got {:?}",
            [first.violations, second.violations]
        );

        clear_command_surface_semantic_snapshot();
    }
}