// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Property tests: tree invariants hold after any sequence of NavActions.

use graph_tree::*;
use proptest::prelude::*;

/// Member IDs are small integers so collisions (duplicate attaches, etc.) exercise
/// edge cases naturally.
const MAX_MEMBER_ID: u64 = 20;

/// Generate a random NavAction over small member IDs.
fn arb_nav_action() -> impl Strategy<Value = NavAction<u64>> {
    prop_oneof![
        // Attach with various provenances
        (1..=MAX_MEMBER_ID).prop_map(|m| NavAction::Attach {
            member: m,
            provenance: Provenance::Anchor,
        }),
        (1..=MAX_MEMBER_ID, 1..=MAX_MEMBER_ID).prop_map(|(m, s)| NavAction::Attach {
            member: m,
            provenance: Provenance::Traversal {
                source: s,
                edge_kind: None,
            },
        }),
        (1..=MAX_MEMBER_ID, 1..=MAX_MEMBER_ID).prop_map(|(m, s)| NavAction::Attach {
            member: m,
            provenance: Provenance::Manual {
                source: Some(s),
                context: None,
            },
        }),
        // Select / Activate / Dismiss
        (1..=MAX_MEMBER_ID).prop_map(NavAction::Select),
        (1..=MAX_MEMBER_ID).prop_map(NavAction::Activate),
        (1..=MAX_MEMBER_ID).prop_map(NavAction::Dismiss),
        // Expand / Reveal
        (1..=MAX_MEMBER_ID).prop_map(NavAction::ToggleExpand),
        (1..=MAX_MEMBER_ID).prop_map(NavAction::Reveal),
        // Detach
        (1..=MAX_MEMBER_ID, any::<bool>()).prop_map(|(m, r)| NavAction::Detach {
            member: m,
            recursive: r,
        }),
        // Reparent
        (1..=MAX_MEMBER_ID, 1..=MAX_MEMBER_ID).prop_map(|(m, p)| NavAction::Reparent {
            member: m,
            new_parent: p,
        }),
        // Lifecycle
        (1..=MAX_MEMBER_ID, prop_oneof![
            Just(Lifecycle::Active),
            Just(Lifecycle::Warm),
            Just(Lifecycle::Cold),
        ]).prop_map(|(m, l)| NavAction::SetLifecycle(m, l)),
        // Layout mode
        prop_oneof![
            Just(LayoutMode::TreeStyleTabs),
            Just(LayoutMode::FlatTabs),
            Just(LayoutMode::SplitPanes),
        ].prop_map(NavAction::SetLayoutMode),
        // Lens
        prop_oneof![
            Just(ProjectionLens::Traversal),
            Just(ProjectionLens::Arrangement),
            Just(ProjectionLens::Containment),
            Just(ProjectionLens::Semantic),
            Just(ProjectionLens::Recency),
            Just(ProjectionLens::All),
        ].prop_map(NavAction::SetLens),
        // Focus cycling
        prop_oneof![
            Just(FocusDirection::Next),
            Just(FocusDirection::Previous),
        ].prop_map(NavAction::CycleFocus),
        prop_oneof![
            Just(FocusCycleRegion::Roots),
            Just(FocusCycleRegion::Branches),
            Just(FocusCycleRegion::Leaves),
        ].prop_map(NavAction::CycleFocusRegion),
    ]
}

/// Verify all tree invariants hold.
fn assert_tree_invariants(tree: &GraphTree<u64>) {
    // 1. Topology invariants (parent/child consistency, no cycles, no duplicates)
    tree.topology().assert_invariants();

    // 2. Every member in the members map that has a topology entry is consistent
    for (id, _) in tree.members() {
        // If member is in topology, depth should not panic
        if tree.topology().contains(id) {
            let _ = tree.depth_of(id);
        }
    }

    // 3. Active member (if any) must exist in members
    if let Some(active) = tree.active() {
        assert!(
            tree.contains(active),
            "active member {:?} not in members map",
            active
        );
    }

    // 4. Member count matches
    assert_eq!(
        tree.member_count(),
        tree.members().count(),
        "member_count disagrees with members iterator"
    );

    // 5. Lifecycle counts are consistent
    assert_eq!(
        tree.active_count() + tree.warm_count() + tree.cold_count(),
        tree.member_count(),
        "lifecycle counts don't sum to member_count"
    );

    // 6. visible_rows doesn't panic
    let _ = tree.visible_rows();

    // 7. compute_layout doesn't panic
    let _ = tree.compute_layout(Rect::new(0.0, 0.0, 800.0, 600.0));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn random_nav_actions_preserve_invariants(
        actions in proptest::collection::vec(arb_nav_action(), 1..40)
    ) {
        let mut tree = GraphTree::new(LayoutMode::TreeStyleTabs, ProjectionLens::Traversal);

        for action in actions {
            tree.apply(action);
            assert_tree_invariants(&tree);
        }
    }

    #[test]
    fn serialization_survives_random_state(
        actions in proptest::collection::vec(arb_nav_action(), 1..30)
    ) {
        let mut tree = GraphTree::new(LayoutMode::TreeStyleTabs, ProjectionLens::Traversal);

        for action in actions {
            tree.apply(action);
        }

        let json = serde_json::to_string(&tree).unwrap();
        let restored: GraphTree<u64> = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.member_count(), tree.member_count());
        assert_tree_invariants(&restored);
    }
}
