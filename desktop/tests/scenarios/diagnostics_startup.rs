use super::super::harness::TestRegistry;
use crate::shell::desktop::runtime::registries;

#[test]
fn startup_env_snapshot_emits_channel_when_env_set() {
    let mut harness = TestRegistry::new();
    let key = "GRAPHSHELL_PERSISTENCE_OPEN_TIMEOUT_MS";
    let prior = std::env::var(key).ok();
    unsafe {
        std::env::set_var(key, "123");
    }

    crate::shell::desktop::runtime::cli::emit_startup_env_snapshot_for_tests();

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_STARTUP_CONFIG_SNAPSHOT) > 0,
        "startup env snapshot should emit config channel"
    );

    match prior {
        Some(value) => unsafe {
            std::env::set_var(key, value);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }
}

#[test]
fn history_manager_limit_env_emits_channel() {
    let mut harness = TestRegistry::new();
    let key = "GRAPHSHELL_HISTORY_MANAGER_LIMIT";
    let prior = std::env::var(key).ok();
    unsafe {
        std::env::set_var(key, "42");
    }

    let _ = crate::render::history_manager_entry_limit_for_tests();

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_UI_HISTORY_MANAGER_LIMIT) > 0,
        "history manager limit should emit diagnostic channel"
    );

    match prior {
        Some(value) => unsafe {
            std::env::set_var(key, value);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }
}

#[test]
fn verse_preinit_calls_emit_channel() {
    let mut harness = TestRegistry::new();

    let _ = crate::mods::native::verse::node_id();

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_PREINIT_CALL) > 0,
        "verse pre-init fallback should emit diagnostic channel"
    );
}
