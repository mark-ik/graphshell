use super::super::harness::TestHarness;
use crate::app::GraphIntent;
use crate::graph::EdgeType;

#[test]
fn create_user_grouped_edge_from_primary_selection_creates_grouped_edge() {
    let mut harness = TestHarness::new();
    let source = harness.add_node("https://a.com");
    let destination = harness.add_node("https://b.com");

    harness.app.select_node(destination, false);
    harness.app.select_node(source, true);

    harness
        .app
        .apply_intents_with_services(crate::app::default_app_services(), [GraphIntent::CreateUserGroupedEdgeFromPrimarySelection]);

    let grouped_edge_count = harness
        .app
        .graph
        .edges()
        .filter(|edge| {
            edge.edge_type == EdgeType::UserGrouped
                && edge.from == source
                && edge.to == destination
        })
        .count();

    assert_eq!(
        grouped_edge_count, 1,
        "grouping action should create a single UserGrouped edge"
    );
}
