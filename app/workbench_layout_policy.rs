use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use crate::shell::desktop::workbench::ux_tree::{UxDomainIdentity, UxTreeSnapshot};
#[cfg(any(feature = "diagnostics", test))]
use crate::shell::desktop::workbench::ux_tree::UxNodeRole;

use super::WorkbenchIntent;
use super::settings_persistence::NavigatorSidebarSidePreference;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AnchorEdge {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SurfaceRole {
    Navigator,
    DiagnosticsPane,
    FacetRail,
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NavigatorHostId {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NavigatorHostScope {
    Both,
    GraphOnly,
    WorkbenchOnly,
    Auto,
}

impl NavigatorHostScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::GraphOnly => "graph",
            Self::WorkbenchOnly => "workbench",
            Self::Auto => "auto",
        }
    }

    pub fn resolve(self, prefer_workbench_scope: bool) -> Self {
        match self {
            Self::Auto => {
                if prefer_workbench_scope {
                    Self::WorkbenchOnly
                } else {
                    Self::GraphOnly
                }
            }
            other => other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SurfaceHostId {
    Navigator(NavigatorHostId),
    Role(SurfaceRole),
}

impl fmt::Display for SurfaceHostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Navigator(host) => write!(f, "Navigator:{}", navigator_host_segment(*host)),
            Self::Role(role) => write!(f, "Role:{}", surface_role_segment(role)),
        }
    }
}

impl FromStr for SurfaceHostId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((prefix, remainder)) = value.split_once(':') else {
            return Err(format!("invalid surface host id '{value}'"));
        };

        match prefix {
            "Navigator" => parse_navigator_host_id(remainder).map(Self::Navigator),
            "Role" => parse_surface_role(remainder).map(Self::Role),
            _ => Err(format!("unknown surface host id prefix '{prefix}'")),
        }
    }
}

impl serde::Serialize for SurfaceHostId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for SurfaceHostId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WorkbenchLayoutConstraint {
    Unconstrained,
    AnchoredSplit {
        surface_host: SurfaceHostId,
        anchor_edge: AnchorEdge,
        anchor_size_fraction: f32,
        cross_axis_margin_start_px: f32,
        cross_axis_margin_end_px: f32,
        resizable: bool,
    },
}

impl WorkbenchLayoutConstraint {
    pub fn anchored_split(
        surface_host: SurfaceHostId,
        anchor_edge: AnchorEdge,
        anchor_size_fraction: f32,
    ) -> Self {
        Self::AnchoredSplit {
            surface_host,
            anchor_edge,
            anchor_size_fraction,
            cross_axis_margin_start_px: 0.0,
            cross_axis_margin_end_px: 0.0,
            resizable: true,
        }
    }

    pub fn surface_host(&self) -> Option<&SurfaceHostId> {
        match self {
            Self::Unconstrained => None,
            Self::AnchoredSplit { surface_host, .. } => Some(surface_host),
        }
    }

    pub fn anchor_edge(&self) -> Option<AnchorEdge> {
        match self {
            Self::Unconstrained => None,
            Self::AnchoredSplit { anchor_edge, .. } => Some(*anchor_edge),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UxConfigMode {
    Locked,
    Configuring { surface_host: SurfaceHostId },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FirstUseOutcome {
    ConfigureNow,
    AcceptDefault,
    Dismissed,
    Discarded,
    RememberedConstraint(WorkbenchLayoutConstraint),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SurfaceFirstUsePolicy {
    pub surface_host: SurfaceHostId,
    pub prompt_shown: bool,
    pub outcome: Option<FirstUseOutcome>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkbenchProfile {
    #[serde(default)]
    pub layout_constraints: HashMap<SurfaceHostId, WorkbenchLayoutConstraint>,
    #[serde(default)]
    pub first_use_policies: HashMap<SurfaceHostId, SurfaceFirstUsePolicy>,
    #[serde(default)]
    pub navigator_host_scopes: HashMap<SurfaceHostId, NavigatorHostScope>,
}

impl WorkbenchProfile {
    pub fn set_layout_constraint(
        &mut self,
        surface_host: SurfaceHostId,
        constraint: WorkbenchLayoutConstraint,
    ) {
        if matches!(constraint, WorkbenchLayoutConstraint::Unconstrained) {
            self.layout_constraints.remove(&surface_host);
        } else {
            self.layout_constraints.insert(surface_host, constraint);
        }
    }

    pub fn set_first_use_policy(&mut self, policy: SurfaceFirstUsePolicy) {
        self.first_use_policies
            .insert(policy.surface_host.clone(), policy);
    }

    pub fn navigator_host_scope(&self, surface_host: &SurfaceHostId) -> NavigatorHostScope {
        self.navigator_host_scopes
            .get(surface_host)
            .copied()
            .unwrap_or_else(|| default_navigator_host_scope(surface_host))
    }

    pub fn set_navigator_host_scope(
        &mut self,
        surface_host: SurfaceHostId,
        scope: NavigatorHostScope,
    ) {
        self.navigator_host_scopes.insert(surface_host, scope);
    }
}

pub fn default_navigator_host_scope(surface_host: &SurfaceHostId) -> NavigatorHostScope {
    match surface_host {
        SurfaceHostId::Navigator(NavigatorHostId::Right) => NavigatorHostScope::Both,
        SurfaceHostId::Navigator(_) | SurfaceHostId::Role(_) => NavigatorHostScope::Both,
    }
}

pub fn default_navigator_surface_host() -> SurfaceHostId {
    SurfaceHostId::Navigator(NavigatorHostId::Left)
}

pub fn navigator_surface_host_for_sidebar_side(
    side: NavigatorSidebarSidePreference,
) -> SurfaceHostId {
    match side {
        NavigatorSidebarSidePreference::Left => SurfaceHostId::Navigator(NavigatorHostId::Left),
        NavigatorSidebarSidePreference::Right => SurfaceHostId::Navigator(NavigatorHostId::Right),
    }
}

pub fn evaluate_layout_policy(
    snapshot: &UxTreeSnapshot,
    profile: &WorkbenchProfile,
) -> Vec<WorkbenchIntent> {
    evaluate_layout_policy_report(snapshot, profile).intents
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutPolicyDiagnostics {
    pub conflict_count: usize,
    pub drift_count: usize,
}

#[derive(Debug, Clone)]
pub struct LayoutPolicyEvaluationReport {
    pub intents: Vec<WorkbenchIntent>,
    pub diagnostics: LayoutPolicyDiagnostics,
}

pub fn evaluate_layout_policy_report(
    snapshot: &UxTreeSnapshot,
    profile: &WorkbenchProfile,
) -> LayoutPolicyEvaluationReport {
    let mut conflicting_edges = HashSet::new();
    let mut claimed_edges = HashMap::<AnchorEdge, SurfaceHostId>::new();

    for (surface_host, constraint) in &profile.layout_constraints {
        let WorkbenchLayoutConstraint::AnchoredSplit { anchor_edge, .. } = constraint else {
            continue;
        };
        if !constraint_matches_live_snapshot(snapshot, surface_host, constraint) {
            continue;
        }
        if let Some(existing) = claimed_edges.insert(*anchor_edge, surface_host.clone()) {
            conflicting_edges.insert(*anchor_edge);
            claimed_edges.insert(*anchor_edge, existing);
        }
    }

    let drift_count = detect_layout_constraint_drift(snapshot, profile, &conflicting_edges);
    let intents = profile
        .layout_constraints
        .iter()
        .filter_map(|(surface_host, constraint)| {
            if matches!(constraint, WorkbenchLayoutConstraint::Unconstrained) {
                return None;
            }
            if !constraint_matches_live_snapshot(snapshot, surface_host, constraint) {
                return None;
            }
            if constraint
                .surface_host()
                .is_some_and(|host| host != surface_host)
            {
                return None;
            }
            if constraint
                .anchor_edge()
                .is_some_and(|edge| conflicting_edges.contains(&edge))
            {
                return None;
            }
            Some(WorkbenchIntent::ApplyLayoutConstraint {
                surface_host: surface_host.clone(),
                constraint: constraint.clone(),
            })
        })
        .collect();

    LayoutPolicyEvaluationReport {
        intents,
        diagnostics: LayoutPolicyDiagnostics {
            conflict_count: conflicting_edges.len(),
            drift_count,
        },
    }
}

fn detect_layout_constraint_drift(
    snapshot: &UxTreeSnapshot,
    profile: &WorkbenchProfile,
    conflicting_edges: &HashSet<AnchorEdge>,
) -> usize {
    profile
        .layout_constraints
        .iter()
        .filter(|(surface_host, constraint)| {
            let WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: constraint_host,
                anchor_edge,
                ..
            } = constraint
            else {
                return false;
            };
            if constraint_host != *surface_host {
                return false;
            }
            if conflicting_edges.contains(anchor_edge)
                || !constraint_matches_live_snapshot(snapshot, surface_host, constraint)
            {
                return false;
            }
            presentation_bounds_for_constraint(snapshot, surface_host, constraint).is_some_and(
                |bounds| {
                    let thickness = match anchor_edge {
                        AnchorEdge::Top | AnchorEdge::Bottom => bounds[3] - bounds[1],
                        AnchorEdge::Left | AnchorEdge::Right => bounds[2] - bounds[0],
                    };
                    thickness <= 2.0
                },
            )
        })
        .count()
}

fn presentation_bounds_for_constraint(
    snapshot: &UxTreeSnapshot,
    surface_host: &SurfaceHostId,
    constraint: &WorkbenchLayoutConstraint,
) -> Option<[f32; 4]> {
    let ux_node_id = matching_live_ux_node_id(snapshot, surface_host, Some(constraint))?;
    snapshot
        .presentation_nodes
        .iter()
        .find(|node| node.ux_node_id == ux_node_id)
        .and_then(|node| node.bounds)
}

fn surface_host_is_live(snapshot: &UxTreeSnapshot, surface_host: &SurfaceHostId) -> bool {
    matching_live_ux_node_id(snapshot, surface_host, None).is_some()
}

fn constraint_matches_live_snapshot(
    snapshot: &UxTreeSnapshot,
    surface_host: &SurfaceHostId,
    constraint: &WorkbenchLayoutConstraint,
) -> bool {
    if constraint
        .surface_host()
        .is_some_and(|host| host != surface_host)
    {
        return false;
    }
    let Some(ux_node_id) = matching_live_ux_node_id(snapshot, surface_host, Some(constraint))
    else {
        return false;
    };
    surface_host_is_layout_eligible(snapshot, ux_node_id)
}

fn matching_live_ux_node_id<'a>(
    snapshot: &'a UxTreeSnapshot,
    surface_host: &SurfaceHostId,
    constraint: Option<&WorkbenchLayoutConstraint>,
) -> Option<&'a str> {
    let expected_edge = constraint.and_then(WorkbenchLayoutConstraint::anchor_edge);
    let expected_form_factor = expected_edge.map(default_form_factor_label_for_edge);

    match surface_host {
        SurfaceHostId::Navigator(host_id) => {
            snapshot
                .semantic_nodes
                .iter()
                .find_map(|node| match &node.domain {
                    UxDomainIdentity::NavigatorProjection {
                        host,
                        anchor_edge,
                        form_factor,
                        ..
                    } if host == surface_host
                        && matches!(
                            host_id,
                            NavigatorHostId::Top
                                | NavigatorHostId::Bottom
                                | NavigatorHostId::Left
                                | NavigatorHostId::Right
                        )
                        && expected_edge.is_none_or(|edge| *anchor_edge == edge)
                        && expected_form_factor
                            .is_none_or(|expected| form_factor.eq_ignore_ascii_case(expected)) =>
                    {
                        Some(node.ux_node_id.as_str())
                    }
                    _ => None,
                })
        }
        SurfaceHostId::Role(SurfaceRole::Navigator) => {
            snapshot
                .semantic_nodes
                .iter()
                .find_map(|node| match node.domain {
                    UxDomainIdentity::NavigatorProjection { .. } => Some(node.ux_node_id.as_str()),
                    _ => None,
                })
        }
        SurfaceHostId::Role(SurfaceRole::DiagnosticsPane) => {
            diagnostics_surface_ux_node_id(snapshot)
        }
        SurfaceHostId::Role(SurfaceRole::FacetRail) => None,
        SurfaceHostId::Role(SurfaceRole::Named(name)) => {
            snapshot.semantic_nodes.iter().find_map(|node| {
                (node.ux_node_id == *name || node.label.eq_ignore_ascii_case(name))
                    .then_some(node.ux_node_id.as_str())
            })
        }
    }
}

#[cfg(feature = "diagnostics")]
fn diagnostics_surface_ux_node_id(snapshot: &UxTreeSnapshot) -> Option<&str> {
    snapshot.semantic_nodes.iter().find_map(|node| {
        matches!(node.role, UxNodeRole::ToolPane).then_some(node.ux_node_id.as_str())
    })
}

#[cfg(not(feature = "diagnostics"))]
fn diagnostics_surface_ux_node_id(_snapshot: &UxTreeSnapshot) -> Option<&str> {
    None
}

fn surface_host_is_layout_eligible(snapshot: &UxTreeSnapshot, ux_node_id: &str) -> bool {
    let Some(node) = snapshot
        .presentation_nodes
        .iter()
        .find(|node| node.ux_node_id == ux_node_id)
    else {
        return true;
    };

    let has_flag = |flag: &str| {
        node.style_flags.iter().any(|candidate| *candidate == flag)
            || node
                .transient_flags
                .iter()
                .any(|candidate| *candidate == flag)
    };

    !(has_flag("lock:fully-locked")
        || has_flag("presentation:floating")
        || has_flag("presentation:fullscreen"))
}

fn default_form_factor_label_for_edge(anchor_edge: AnchorEdge) -> &'static str {
    match anchor_edge {
        AnchorEdge::Top | AnchorEdge::Bottom => "toolbar",
        AnchorEdge::Left | AnchorEdge::Right => "sidebar",
    }
}

fn navigator_host_segment(host: NavigatorHostId) -> &'static str {
    match host {
        NavigatorHostId::Top => "Top",
        NavigatorHostId::Bottom => "Bottom",
        NavigatorHostId::Left => "Left",
        NavigatorHostId::Right => "Right",
    }
}

fn parse_navigator_host_id(value: &str) -> Result<NavigatorHostId, String> {
    match value {
        "Top" => Ok(NavigatorHostId::Top),
        "Bottom" => Ok(NavigatorHostId::Bottom),
        "Left" => Ok(NavigatorHostId::Left),
        "Right" => Ok(NavigatorHostId::Right),
        _ => Err(format!("unknown navigator host id '{value}'")),
    }
}

fn surface_role_segment(role: &SurfaceRole) -> String {
    match role {
        SurfaceRole::Navigator => "Navigator".to_string(),
        SurfaceRole::DiagnosticsPane => "DiagnosticsPane".to_string(),
        SurfaceRole::FacetRail => "FacetRail".to_string(),
        SurfaceRole::Named(name) => format!("Named:{name}"),
    }
}

fn parse_surface_role(value: &str) -> Result<SurfaceRole, String> {
    match value {
        "Navigator" => Ok(SurfaceRole::Navigator),
        "DiagnosticsPane" => Ok(SurfaceRole::DiagnosticsPane),
        "FacetRail" => Ok(SurfaceRole::FacetRail),
        _ => value
            .strip_prefix("Named:")
            .map(|name| SurfaceRole::Named(name.to_string()))
            .ok_or_else(|| format!("unknown surface role '{value}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "diagnostics")]
    use crate::app::runtime_ports::diagnostics::DiagnosticsState;
    use crate::shell::desktop::workbench::ux_tree::{
        UxAction, UxNodeState, UxPresentationNode, UxSemanticNode, UxTraceSummary,
    };

    #[cfg(feature = "diagnostics")]
    fn channel_count(snapshot: &serde_json::Value, channel: &str) -> u64 {
        snapshot
            .get("channels")
            .and_then(|channels| channels.get("message_counts"))
            .and_then(|counts| counts.get(channel))
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    }

    fn navigator_snapshot_for_hosts(hosts: &[NavigatorHostId]) -> UxTreeSnapshot {
        UxTreeSnapshot {
            semantic_version: 1,
            presentation_version: 1,
            trace_version: 1,
            semantic_nodes: hosts
                .iter()
                .map(|host| {
                    let anchor_edge = match host {
                        NavigatorHostId::Top => AnchorEdge::Top,
                        NavigatorHostId::Bottom => AnchorEdge::Bottom,
                        NavigatorHostId::Left => AnchorEdge::Left,
                        NavigatorHostId::Right => AnchorEdge::Right,
                    };
                    UxSemanticNode {
                        ux_node_id: format!("uxnode://navigator/{host:?}"),
                        parent_ux_node_id: None,
                        role: UxNodeRole::NavigatorProjection,
                        label: "Navigator".to_string(),
                        state: UxNodeState {
                            focused: false,
                            selected: false,
                            blocked: false,
                            degraded: false,
                        },
                        allowed_actions: vec![UxAction::Focus],
                        domain: UxDomainIdentity::NavigatorProjection {
                            host: SurfaceHostId::Navigator(*host),
                            anchor_edge,
                            form_factor: match anchor_edge {
                                AnchorEdge::Top | AnchorEdge::Bottom => "toolbar".to_string(),
                                AnchorEdge::Left | AnchorEdge::Right => "sidebar".to_string(),
                            },
                            scope: "workbench".to_string(),
                            projection_mode: "Workbench".to_string(),
                            projection_seed_source: "graph-containment".to_string(),
                            sort_mode: "manual".to_string(),
                            root_filter: None,
                            row_count: 1,
                            selected_count: 0,
                            expanded_count: 0,
                            collapsed_count: 0,
                            workbench_group_count: 0,
                            workbench_member_count: 0,
                            unrelated_count: 0,
                            recent_count: 0,
                        },
                    }
                })
                .collect(),
            presentation_nodes: Vec::new(),
            trace_nodes: Vec::new(),
            trace_summary: UxTraceSummary {
                build_duration_us: 0,
                route_events_observed: 0,
                diagnostics_events_observed: 0,
            },
        }
    }

    fn navigator_snapshot_for_host_edges(
        hosts: &[(NavigatorHostId, AnchorEdge)],
    ) -> UxTreeSnapshot {
        UxTreeSnapshot {
            semantic_version: 1,
            presentation_version: 1,
            trace_version: 1,
            semantic_nodes: hosts
                .iter()
                .map(|(host, anchor_edge)| UxSemanticNode {
                    ux_node_id: format!("uxnode://navigator/{host:?}/{anchor_edge:?}"),
                    parent_ux_node_id: None,
                    role: UxNodeRole::NavigatorProjection,
                    label: "Navigator".to_string(),
                    state: UxNodeState {
                        focused: false,
                        selected: false,
                        blocked: false,
                        degraded: false,
                    },
                    allowed_actions: vec![UxAction::Focus],
                    domain: UxDomainIdentity::NavigatorProjection {
                        host: SurfaceHostId::Navigator(*host),
                        anchor_edge: *anchor_edge,
                        form_factor: default_form_factor_label_for_edge(*anchor_edge).to_string(),
                        scope: "workbench".to_string(),
                        projection_mode: "Workbench".to_string(),
                        projection_seed_source: "graph-containment".to_string(),
                        sort_mode: "manual".to_string(),
                        root_filter: None,
                        row_count: 1,
                        selected_count: 0,
                        expanded_count: 0,
                        collapsed_count: 0,
                        workbench_group_count: 0,
                        workbench_member_count: 0,
                        unrelated_count: 0,
                        recent_count: 0,
                    },
                })
                .collect(),
            presentation_nodes: Vec::new(),
            trace_nodes: Vec::new(),
            trace_summary: UxTraceSummary {
                build_duration_us: 0,
                route_events_observed: 0,
                diagnostics_events_observed: 0,
            },
        }
    }

    fn named_surface_snapshot(
        surface_name: &str,
        style_flags: Vec<&'static str>,
        transient_flags: Vec<&'static str>,
    ) -> UxTreeSnapshot {
        UxTreeSnapshot {
            semantic_version: 1,
            presentation_version: 1,
            trace_version: 1,
            semantic_nodes: vec![UxSemanticNode {
                ux_node_id: surface_name.to_string(),
                parent_ux_node_id: None,
                role: UxNodeRole::NodePane,
                label: surface_name.to_string(),
                state: UxNodeState {
                    focused: false,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Node {
                    node_key: crate::graph::NodeKey::new(1),
                    pane_id: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                    lifecycle: crate::graph::NodeLifecycle::Cold,
                    attach_attempt: None,
                },
            }],
            presentation_nodes: vec![UxPresentationNode {
                ux_node_id: surface_name.to_string(),
                bounds: Some([0.0, 0.0, 300.0, 180.0]),
                render_mode: None,
                z_pass: "workbench.content",
                style_flags,
                transient_flags,
            }],
            trace_nodes: Vec::new(),
            trace_summary: UxTraceSummary {
                build_duration_us: 0,
                route_events_observed: 0,
                diagnostics_events_observed: 0,
            },
        }
    }

    #[test]
    fn evaluate_layout_policy_emits_apply_intent_for_live_navigator_host() {
        let surface_host = SurfaceHostId::Navigator(NavigatorHostId::Top);
        let constraint =
            WorkbenchLayoutConstraint::anchored_split(surface_host.clone(), AnchorEdge::Top, 0.25);
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(surface_host.clone(), constraint.clone());

        let intents = evaluate_layout_policy(
            &navigator_snapshot_for_hosts(&[NavigatorHostId::Top]),
            &profile,
        );

        assert!(matches!(
            intents.as_slice(),
            [WorkbenchIntent::ApplyLayoutConstraint {
                surface_host: emitted_surface_host,
                constraint: emitted_constraint,
            }] if emitted_surface_host == &surface_host && emitted_constraint == &constraint
        ));
    }

    #[test]
    fn evaluate_layout_policy_skips_conflicting_anchor_edges() {
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(
            SurfaceHostId::Navigator(NavigatorHostId::Top),
            WorkbenchLayoutConstraint::anchored_split(
                SurfaceHostId::Navigator(NavigatorHostId::Top),
                AnchorEdge::Top,
                0.2,
            ),
        );
        profile.set_layout_constraint(
            SurfaceHostId::Navigator(NavigatorHostId::Bottom),
            WorkbenchLayoutConstraint::anchored_split(
                SurfaceHostId::Navigator(NavigatorHostId::Bottom),
                AnchorEdge::Top,
                0.3,
            ),
        );

        let intents = evaluate_layout_policy(
            &navigator_snapshot_for_host_edges(&[
                (NavigatorHostId::Top, AnchorEdge::Top),
                (NavigatorHostId::Bottom, AnchorEdge::Top),
            ]),
            &profile,
        );

        assert!(intents.is_empty());
    }

    #[test]
    fn evaluate_layout_policy_skips_navigator_constraint_when_host_does_not_match_live_snapshot() {
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(
            SurfaceHostId::Navigator(NavigatorHostId::Bottom),
            WorkbenchLayoutConstraint::anchored_split(
                SurfaceHostId::Navigator(NavigatorHostId::Bottom),
                AnchorEdge::Bottom,
                0.22,
            ),
        );

        let intents = evaluate_layout_policy(
            &navigator_snapshot_for_hosts(&[NavigatorHostId::Top]),
            &profile,
        );

        assert!(intents.is_empty());
    }

    #[test]
    fn surface_host_id_round_trips_stable_string_format() {
        let value = SurfaceHostId::Role(SurfaceRole::Named("aux-pane".to_string()));
        let encoded = value.to_string();
        let decoded = SurfaceHostId::from_str(&encoded).expect("surface host id should parse");
        assert_eq!(decoded, value);
        assert_eq!(encoded, "Role:Named:aux-pane");
    }

    #[test]
    fn evaluate_layout_policy_report_skips_navigator_constraint_when_live_anchor_metadata_differs()
    {
        let surface_host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        let constraint =
            WorkbenchLayoutConstraint::anchored_split(surface_host.clone(), AnchorEdge::Top, 0.2);
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(surface_host, constraint);

        let report = evaluate_layout_policy_report(
            &navigator_snapshot_for_hosts(&[NavigatorHostId::Right]),
            &profile,
        );

        assert!(report.intents.is_empty());
        assert_eq!(report.diagnostics.conflict_count, 0);
        assert_eq!(report.diagnostics.drift_count, 0);
    }

    #[test]
    fn evaluate_layout_policy_report_excludes_fully_locked_named_surface() {
        let surface_host = SurfaceHostId::Role(SurfaceRole::Named("locked-pane".to_string()));
        let constraint =
            WorkbenchLayoutConstraint::anchored_split(surface_host.clone(), AnchorEdge::Left, 0.25);
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(surface_host, constraint);

        let report = evaluate_layout_policy_report(
            &named_surface_snapshot(
                "locked-pane",
                vec!["surface:node"],
                vec!["lock:fully-locked"],
            ),
            &profile,
        );

        assert!(report.intents.is_empty());
    }

    #[test]
    fn evaluate_layout_policy_report_excludes_floating_named_surface() {
        let surface_host = SurfaceHostId::Role(SurfaceRole::Named("floating-pane".to_string()));
        let constraint =
            WorkbenchLayoutConstraint::anchored_split(surface_host.clone(), AnchorEdge::Right, 0.2);
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(surface_host, constraint);

        let report = evaluate_layout_policy_report(
            &named_surface_snapshot(
                "floating-pane",
                vec!["surface:node", "presentation:floating"],
                Vec::new(),
            ),
            &profile,
        );

        assert!(report.intents.is_empty());
    }

    #[test]
    fn evaluate_layout_policy_report_excludes_fullscreen_named_surface() {
        let surface_host = SurfaceHostId::Role(SurfaceRole::Named("fullscreen-pane".to_string()));
        let constraint = WorkbenchLayoutConstraint::anchored_split(
            surface_host.clone(),
            AnchorEdge::Bottom,
            0.3,
        );
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(surface_host, constraint);

        let report = evaluate_layout_policy_report(
            &named_surface_snapshot(
                "fullscreen-pane",
                vec!["surface:node", "presentation:fullscreen"],
                Vec::new(),
            ),
            &profile,
        );

        assert!(report.intents.is_empty());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn evaluate_layout_policy_remains_pure_and_does_not_emit_diagnostics() {
        let mut diagnostics = DiagnosticsState::new();
        let surface_host = SurfaceHostId::Navigator(NavigatorHostId::Top);
        let mut profile = WorkbenchProfile::default();
        profile.set_layout_constraint(
            surface_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(surface_host, AnchorEdge::Top, 0.25),
        );

        let _ = evaluate_layout_policy(
            &navigator_snapshot_for_hosts(&[NavigatorHostId::Top]),
            &profile,
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert_eq!(
            channel_count(
                &snapshot,
                crate::app::runtime_ports::registries::CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT
            ),
            0
        );
        assert_eq!(
            channel_count(
                &snapshot,
                crate::app::runtime_ports::registries::CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT
            ),
            0
        );
    }
}
