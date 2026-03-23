/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas physics scenario tests P1–P3.
//!
//! Implements the canonical scenario assertions from:
//! `design_docs/graphshell_docs/implementation_strategy/canvas/2026-03-14_canvas_behavior_contract.md`
//!
//! These tests are headless (no egui::Ui / render context) and use a fixed
//! canvas rect. They validate physics convergence, preset ordering, and
//! structural invariants before any force additions (frame-affinity, etc.)
//! can mask regressions.
//!
//! Scenarios P1–P3 gate the preset ordering invariant (spec §4):
//!   mean_edge_length(Solid) ≤ mean_edge_length(Liquid) ≤ mean_edge_length(Gas)
//!   mean_edge_length(Gas) ≥ mean_edge_length(Solid) × 1.25

use euclid::default::Point2D;
use std::collections::HashSet;

use super::barnes_hut_force_directed::BarnesHutForceDirectedLayout;
use crate::graph::physics::{default_graph_physics_state, scenario_helpers::*};
use crate::model::graph::Graph;
use crate::model::graph::apply::{GraphDelta, GraphDeltaResult, apply_graph_delta};
use crate::model::graph::egui_adapter::EguiGraphState;
use crate::registries::atomic::lens::PhysicsProfile;

/// Convergence threshold: average displacement < this value = converged.
/// Corresponds to spec §2.1 `convergence_threshold` (mapped from KE proxy).
const CONVERGENCE_THRESHOLD: f32 = 0.5;

/// Consecutive steps below threshold required to declare convergence (spec §2.1).
const CONVERGENCE_WINDOW: usize = 10;

/// Default node radius for overlap tests (spec §2.2).
const NODE_RADIUS: f32 = 16.0;

/// Overlap margin (spec §2.2 default 4.0 px).
const OVERLAP_MARGIN: f32 = 4.0;

/// Build a ring graph of `n` nodes: 0→1→2→…→(n-1)→0, all Hyperlink edges.
/// Nodes are placed in a small circle to give physics a reasonable start.
fn build_ring_graph(n: usize) -> Graph {
    let mut graph = Graph::new();
    let mut keys = Vec::new();

    for i in 0..n {
        let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
        let x = 100.0 * angle.cos();
        let y = 100.0 * angle.sin();
        let GraphDeltaResult::NodeAdded(key) = apply_graph_delta(
            &mut graph,
            GraphDelta::AddNode {
                id: None,
                url: format!("https://scenario.test/node/{i}"),
                position: Point2D::new(x, y),
            },
        ) else {
            panic!("expected NodeAdded");
        };
        keys.push(key);
    }

    for i in 0..n {
        let from = keys[i];
        let to = keys[(i + 1) % n];
        apply_graph_delta(
            &mut graph,
            GraphDelta::AssertRelation {
                from,
                to,
                assertion: crate::graph::EdgeAssertion::Semantic {
                    sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                    label: None,
                    decay_progress: None,
                },
            },
        );
    }

    graph
}

/// Run the Barnes-Hut layout headlessly for `max_steps` using `profile`.
///
/// Returns the `EguiGraph` at the end of the run plus the step count at which
/// convergence was reached (or `None` if it did not converge).
fn run_scenario(
    graph: &Graph,
    profile: &PhysicsProfile,
    max_steps: usize,
) -> (crate::model::graph::egui_adapter::EguiGraph, Option<usize>) {
    let mut egui_state = EguiGraphState::from_graph(graph, &HashSet::new());
    let mut physics = default_graph_physics_state();
    physics.base.is_running = true;
    profile.apply_to_state(&mut physics);
    // Restore step parameters that profile.apply_to_state may not set
    physics.base.k_scale = 0.42;
    physics.base.dt = 0.03;
    physics.base.max_step = 3.0;
    // Enable center gravity for all scenarios (as in production default)
    physics.extras.0.enabled = true;
    physics.extras.0.params.c = profile.gravity_strength;

    let mut layout = BarnesHutForceDirectedLayout::new_from_state(physics);
    let rect = test_canvas();

    let mut converge_window = 0;
    let mut converged_at = None;

    for step in 0..max_steps {
        layout.step_with_rect(&mut egui_state.graph, rect);
        if is_converged(layout.physics_state(), CONVERGENCE_THRESHOLD) {
            converge_window += 1;
            if converge_window >= CONVERGENCE_WINDOW && converged_at.is_none() {
                converged_at = Some(step);
            }
        } else {
            converge_window = 0;
        }
    }

    (egui_state.graph, converged_at)
}

/// P1 — Small connected ring, Solid preset.
///
/// Spec: `canvas_behavior_contract.md §3 Scenario P1`
/// Assertions: no overlap, converges within 800 steps, edge_len_cv ≤ 0.45.
#[test]
fn p1_solid_ring_converges_no_overlap_tight_cv() {
    let graph = build_ring_graph(6);
    let profile = PhysicsProfile::solid();
    let (egui_graph, converged_at) = run_scenario(&graph, &profile, 800);

    assert!(
        converged_at.is_some(),
        "P1: Solid ring must converge within 800 steps; physics may be oscillating or over-damped"
    );
    assert_eq!(
        overlap_count(&egui_graph, NODE_RADIUS, OVERLAP_MARGIN),
        0,
        "P1: Solid ring must have zero node overlaps at convergence"
    );
    let cv = edge_len_cv(&egui_graph);
    assert!(
        cv <= 0.45,
        "P1: Solid ring edge_len_cv {cv:.3} must be ≤ 0.45 (spec §2.4 small-graph target)"
    );
}

/// P2 — Small connected ring, Gas preset.
///
/// Spec: `canvas_behavior_contract.md §3 Scenario P2`
/// Assertions: no overlap, converges within 1200 steps, edge_len_cv ≤ 0.65,
/// mean edge length in Gas ≥ mean edge length in Solid × 1.3.
#[test]
fn p2_gas_ring_spreads_wider_than_solid() {
    let graph = build_ring_graph(6);
    let solid_profile = PhysicsProfile::solid();
    let gas_profile = PhysicsProfile::gas();

    let (solid_graph, solid_converged) = run_scenario(&graph, &solid_profile, 800);
    let (gas_graph, gas_converged) = run_scenario(&graph, &gas_profile, 1200);

    assert!(
        solid_converged.is_some(),
        "P2: Solid baseline must converge within 800 steps"
    );
    assert!(
        gas_converged.is_some(),
        "P2: Gas ring must converge within 1200 steps; physics may be diverging"
    );
    assert_eq!(
        overlap_count(&gas_graph, NODE_RADIUS, OVERLAP_MARGIN),
        0,
        "P2: Gas ring must have zero node overlaps at convergence"
    );
    let cv = edge_len_cv(&gas_graph);
    assert!(
        cv <= 0.65,
        "P2: Gas ring edge_len_cv {cv:.3} must be ≤ 0.65 (full portfolio threshold)"
    );

    let solid_mean = mean_edge_length(&solid_graph);
    let gas_mean = mean_edge_length(&gas_graph);
    assert!(
        gas_mean >= solid_mean * 1.3,
        "P2: Gas mean edge length {gas_mean:.1} must be ≥ Solid {solid_mean:.1} × 1.3 = {:.1}; \
         presets are not meaningfully differentiated (spec §3 P2 assertion)",
        solid_mean * 1.3
    );
}

/// P3 — Small connected ring, Liquid preset.
///
/// Spec: `canvas_behavior_contract.md §3 Scenario P3`
/// Assertions: no overlap, converges within 1000 steps,
/// mean edge length Liquid is between Solid and Gas (ordering invariant).
#[test]
fn p3_liquid_ring_is_intermediate_between_solid_and_gas() {
    let graph = build_ring_graph(6);
    let solid_profile = PhysicsProfile::solid();
    let liquid_profile = PhysicsProfile::liquid();
    let gas_profile = PhysicsProfile::gas();

    let (solid_graph, solid_converged) = run_scenario(&graph, &solid_profile, 800);
    let (liquid_graph, liquid_converged) = run_scenario(&graph, &liquid_profile, 1000);
    let (gas_graph, gas_converged) = run_scenario(&graph, &gas_profile, 1200);

    assert!(
        solid_converged.is_some(),
        "P3: Solid baseline must converge"
    );
    assert!(
        liquid_converged.is_some(),
        "P3: Liquid ring must converge within 1000 steps"
    );
    assert!(gas_converged.is_some(), "P3: Gas baseline must converge");

    assert_eq!(
        overlap_count(&liquid_graph, NODE_RADIUS, OVERLAP_MARGIN),
        0,
        "P3: Liquid ring must have zero node overlaps at convergence"
    );

    let solid_mean = mean_edge_length(&solid_graph);
    let liquid_mean = mean_edge_length(&liquid_graph);
    let gas_mean = mean_edge_length(&gas_graph);

    assert!(
        liquid_mean >= solid_mean,
        "P3: Liquid mean edge length {liquid_mean:.1} must be ≥ Solid {solid_mean:.1} \
         (ordering invariant: Solid tightest → Liquid middle → Gas loosest)"
    );
    assert!(
        liquid_mean <= gas_mean,
        "P3: Liquid mean edge length {liquid_mean:.1} must be ≤ Gas {gas_mean:.1} \
         (ordering invariant: Solid tightest → Liquid middle → Gas loosest)"
    );

    // Preset ordering invariant (spec §4): Gas ≥ Solid × 1.25
    assert!(
        gas_mean >= solid_mean * 1.25,
        "Preset ordering invariant: Gas mean {gas_mean:.1} must be ≥ Solid {solid_mean:.1} × 1.25 = {:.1}",
        solid_mean * 1.25
    );
}
