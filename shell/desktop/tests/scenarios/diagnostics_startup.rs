use super::super::harness::TestRegistry;
use crate::shell::desktop::runtime::registries;
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_env_var<R>(key: &str, value: &str, run: impl FnOnce() -> R) -> R {
    let _guard = env_lock().lock().expect("env lock poisoned");
    let prior = std::env::var(key).ok();

    unsafe {
        std::env::set_var(key, value);
    }

    let result = run();

    match prior {
        Some(previous) => unsafe {
            std::env::set_var(key, previous);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }

    result
}

#[test]
fn startup_env_snapshot_emits_channel_when_env_set() {
    with_env_var("GRAPHSHELL_PERSISTENCE_OPEN_TIMEOUT_MS", "123", || {
        let mut harness = TestRegistry::new();

        crate::shell::desktop::runtime::cli::emit_startup_env_snapshot_for_tests();

        let snapshot = harness.snapshot();
        assert!(
            TestRegistry::channel_count(&snapshot, registries::CHANNEL_STARTUP_CONFIG_SNAPSHOT) > 0,
            "startup env snapshot should emit config channel"
        );
    });
}

#[test]
fn history_manager_limit_env_emits_channel() {
    with_env_var("GRAPHSHELL_HISTORY_MANAGER_LIMIT", "42", || {
        let mut harness = TestRegistry::new();

        let _ = crate::render::history_manager_entry_limit_for_tests();

        let snapshot = harness.snapshot();
        assert!(
            TestRegistry::channel_count(&snapshot, registries::CHANNEL_UI_HISTORY_MANAGER_LIMIT) > 0,
            "history manager limit should emit diagnostic channel"
        );
    });
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
