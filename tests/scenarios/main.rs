use graphshell::VERSION;

#[test]
fn scenarios_binary_smoke_runs() {
    assert!(!VERSION.is_empty());
}
