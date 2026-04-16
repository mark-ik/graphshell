use graphshell::VERSION;

#[test]
fn scenarios_binary_smoke_runs() {
    assert!(!VERSION.is_empty());
}

#[test]
fn mod_capability_disable_verso_scenario() {
    let capabilities = graphshell::test_utils::active_capabilities_with_disabled(&["mod:verso"]);

    assert!(!capabilities.contains("viewer:webview"));
    assert!(!capabilities.contains("protocol:https"));
    assert!(capabilities.contains("ProtocolRegistry"));
    assert!(capabilities.contains("ViewerRegistry"));
}
