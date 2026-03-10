pub(crate) mod action;
pub(crate) mod identity;
pub(crate) mod input;
pub(crate) mod knowledge;
pub(crate) mod lens;
pub(crate) mod nostr_core;
pub(crate) mod protocol;
pub(crate) mod renderer;
pub(crate) mod signal_routing;

use std::sync::{Mutex, OnceLock};

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphMutation, MemoryPressureLevel, RendererId, RuntimeEvent,
};
use crate::graph::NodeKey;
use crate::registries::atomic::ProtocolHandlerProviders;
use crate::registries::atomic::ViewerHandlerProviders;
use crate::registries::atomic::diagnostics;
use crate::registries::atomic::lens::LensRegistry;
use crate::registries::atomic::protocol::ProtocolContractRegistry;
use crate::registries::atomic::viewer::{ViewerRegistry, ViewerSelection};
use crate::registries::domain::layout::ConformanceLevel;
use crate::registries::domain::layout::LayoutDomainRegistry;
use crate::registries::domain::layout::viewer_surface::{
    VIEWER_SURFACE_DEFAULT, ViewerSurfaceResolution,
};
use crate::registries::infrastructure::ModRegistry;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::workbench::pane_model::PaneId;
use action::{
    ACTION_DETAIL_VIEW_SUBMIT, ACTION_GRAPH_VIEW_SUBMIT, ACTION_OMNIBOX_NODE_SEARCH,
    ACTION_VERSE_FORGET_DEVICE, ACTION_VERSE_PAIR_DEVICE, ACTION_VERSE_SHARE_WORKSPACE,
    ACTION_VERSE_SYNC_NOW, ActionPayload, ActionRegistry, PairingMode,
};
use diagnostics::DiagnosticsRegistry;
use identity::IdentityRegistry;
use input::{
    INPUT_BINDING_TOOLBAR_SUBMIT, InputBinding, InputBindingRemap, InputContext,
    InputConflict as InputRemapConflict, InputRegistry,
};
use knowledge::KnowledgeRegistry;
use nostr_core::{
    NostrCoreError, NostrCoreRegistry, NostrFilterSet, NostrPublishReceipt, NostrSignedEvent,
    NostrSubscriptionHandle, NostrUnsignedEvent,
};
use protocol::{
    ProtocolRegistry, ProtocolResolution, ProtocolResolveControl, ProtocolResolveOutcome,
};
use renderer::{PaneAttachment, RendererRegistry, RendererRegistryError};
use servo::ServoUrl;
use signal_routing::{
    ObserverId, SignalEnvelope, SignalKind, SignalRoutingLayer, SignalSource, SignalTopic,
};

pub(crate) const CHANNEL_PROTOCOL_RESOLVE_STARTED: &str = "registry.protocol.resolve_started";
pub(crate) const CHANNEL_PROTOCOL_RESOLVE_SUCCEEDED: &str = "registry.protocol.resolve_succeeded";
pub(crate) const CHANNEL_PROTOCOL_RESOLVE_FAILED: &str = "registry.protocol.resolve_failed";
pub(crate) const CHANNEL_PROTOCOL_RESOLVE_FALLBACK_USED: &str = "registry.protocol.fallback_used";
pub(crate) const CHANNEL_VIEWER_SELECT_STARTED: &str = "registry.viewer.select_started";
pub(crate) const CHANNEL_VIEWER_SELECT_SUCCEEDED: &str = "registry.viewer.select_succeeded";
pub(crate) const CHANNEL_VIEWER_FALLBACK_USED: &str = "registry.viewer.fallback_used";
pub(crate) const CHANNEL_VIEWER_FALLBACK_WRY_FEATURE_DISABLED: &str =
    "registry.viewer.fallback_wry_feature_disabled";
pub(crate) const CHANNEL_VIEWER_FALLBACK_WRY_CAPABILITY_MISSING: &str =
    "registry.viewer.fallback_wry_capability_missing";
pub(crate) const CHANNEL_VIEWER_FALLBACK_WRY_DISABLED_BY_PREFERENCE: &str =
    "registry.viewer.fallback_wry_disabled_by_preference";
pub(crate) const CHANNEL_VIEWER_CAPABILITY_PARTIAL: &str = "registry.viewer.capability_partial";
pub(crate) const CHANNEL_VIEWER_CAPABILITY_NONE: &str = "registry.viewer.capability_none";
pub(crate) const CHANNEL_SURFACE_CONFORMANCE_PARTIAL: &str = "registry.surface.conformance_partial";
pub(crate) const CHANNEL_SURFACE_CONFORMANCE_NONE: &str = "registry.surface.conformance_none";
pub(crate) const CHANNEL_ACTION_EXECUTE_STARTED: &str = "registry.action.execute_started";
pub(crate) const CHANNEL_ACTION_EXECUTE_SUCCEEDED: &str = "registry.action.execute_succeeded";
pub(crate) const CHANNEL_ACTION_EXECUTE_FAILED: &str = "registry.action.execute_failed";
pub(crate) const CHANNEL_INPUT_BINDING_RESOLVED: &str = "registry.input.binding_resolved";
pub(crate) const CHANNEL_INPUT_BINDING_MISSING: &str = "registry.input.binding_missing";
pub(crate) const CHANNEL_INPUT_BINDING_CONFLICT: &str = "registry.input.binding_conflict";
pub(crate) const CHANNEL_INPUT_BINDING_REBOUND: &str = "registry.input.binding_rebound";
pub(crate) const CHANNEL_RENDERER_ATTACH: &str = "registry.renderer.attach";
pub(crate) const CHANNEL_RENDERER_DETACH: &str = "registry.renderer.detach";
pub(crate) const CHANNEL_LENS_RESOLVE_SUCCEEDED: &str = "registry.lens.resolve_succeeded";
pub(crate) const CHANNEL_LENS_RESOLVE_FAILED: &str = "registry.lens.resolve_failed";
pub(crate) const CHANNEL_LENS_FALLBACK_USED: &str = "registry.lens.fallback_used";
pub(crate) const CHANNEL_IDENTITY_SIGN_STARTED: &str = "registry.identity.sign_started";
pub(crate) const CHANNEL_IDENTITY_SIGN_SUCCEEDED: &str = "registry.identity.sign_succeeded";
pub(crate) const CHANNEL_IDENTITY_SIGN_FAILED: &str = "registry.identity.sign_failed";
pub(crate) const CHANNEL_IDENTITY_KEY_UNAVAILABLE: &str = "registry.identity.key_unavailable";
pub(crate) const CHANNEL_DIAGNOSTICS_CHANNEL_REGISTERED: &str =
    "registry.diagnostics.channel_registered";
pub(crate) const CHANNEL_DIAGNOSTICS_CONFIG_CHANGED: &str = "registry.diagnostics.config_changed";
pub(crate) const CHANNEL_INVARIANT_TIMEOUT: &str = "registry.invariant.timeout";
pub(crate) const CHANNEL_MOD_LOAD_STARTED: &str = "registry.mod.load_started";
pub(crate) const CHANNEL_MOD_LOAD_SUCCEEDED: &str = "registry.mod.load_succeeded";
pub(crate) const CHANNEL_MOD_LOAD_FAILED: &str = "registry.mod.load_failed";
pub(crate) const CHANNEL_MOD_DEPENDENCY_MISSING: &str = "registry.mod.dependency_missing";
pub(crate) const CHANNEL_STARTUP_CONFIG_SNAPSHOT: &str = "startup.config.snapshot";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED: &str =
    "startup.persistence.open_started";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED: &str =
    "startup.persistence.open_succeeded";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED: &str = "startup.persistence.open_failed";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT: &str =
    "startup.persistence.open_timeout";
pub(crate) const CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED: &str = "persistence.recover.succeeded";
pub(crate) const CHANNEL_PERSISTENCE_RECOVER_FAILED: &str = "persistence.recover.failed";
pub(crate) const CHANNEL_STARTUP_VERSE_INIT_MODE: &str = "startup.verse.init_mode";
pub(crate) const CHANNEL_STARTUP_VERSE_INIT_SUCCEEDED: &str = "startup.verse.init_succeeded";
pub(crate) const CHANNEL_STARTUP_VERSE_INIT_FAILED: &str = "startup.verse.init_failed";
pub(crate) const CHANNEL_STARTUP_SELFCHECK_REGISTRIES_LOADED: &str =
    "startup.selfcheck.registries_loaded";
pub(crate) const CHANNEL_STARTUP_SELFCHECK_CHANNELS_COMPLETE: &str =
    "startup.selfcheck.channels_complete";
pub(crate) const CHANNEL_STARTUP_SELFCHECK_CHANNELS_INCOMPLETE: &str =
    "startup.selfcheck.channels_incomplete";
pub(crate) const CHANNEL_UI_HISTORY_MANAGER_LIMIT: &str = "ui.history_manager.limit_applied";
pub(crate) const CHANNEL_HISTORY_TRAVERSAL_RECORDED: &str = "history.traversal.recorded";
pub(crate) const CHANNEL_HISTORY_TRAVERSAL_RECORD_FAILED: &str = "history.traversal.record_failed";
pub(crate) const CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED: &str =
    "history.archive.dissolved_appended";
pub(crate) const CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED: &str = "history.archive.clear_failed";
pub(crate) const CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED: &str = "history.archive.export_failed";
pub(crate) const CHANNEL_HISTORY_TIMELINE_PREVIEW_ENTERED: &str =
    "history.timeline.preview_entered";
pub(crate) const CHANNEL_HISTORY_TIMELINE_PREVIEW_EXITED: &str = "history.timeline.preview_exited";
pub(crate) const CHANNEL_HISTORY_TIMELINE_PREVIEW_ISOLATION_VIOLATION: &str =
    "history.timeline.preview_isolation_violation";
pub(crate) const CHANNEL_HISTORY_TIMELINE_REPLAY_STARTED: &str = "history.timeline.replay_started";
pub(crate) const CHANNEL_HISTORY_TIMELINE_REPLAY_SUCCEEDED: &str =
    "history.timeline.replay_succeeded";
pub(crate) const CHANNEL_HISTORY_TIMELINE_REPLAY_FAILED: &str = "history.timeline.replay_failed";
pub(crate) const CHANNEL_HISTORY_TIMELINE_RETURN_TO_PRESENT_FAILED: &str =
    "history.timeline.return_to_present_failed";
pub(crate) const CHANNEL_UI_CLIPBOARD_COPY_FAILED: &str = "ui.clipboard.copy_failed";
pub(crate) const CHANNEL_UI_GRAPH_CAMERA_REQUEST_BLOCKED: &str =
    "runtime.ui.graph.camera_request_blocked";
pub(crate) const CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED: &str =
    "runtime.ui.graph.keyboard_zoom_blocked";
pub(crate) const CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_ZERO_VIEW: &str =
    "runtime.ui.graph.camera_fit_blocked_zero_view";
pub(crate) const CHANNEL_UI_GRAPH_FIT_SELECTION_FALLBACK_TO_FIT: &str =
    "runtime.ui.graph.fit_selection_fallback_to_fit";
pub(crate) const CHANNEL_UI_GRAPH_CAMERA_FIT_BLOCKED_NO_BOUNDS: &str =
    "runtime.ui.graph.camera_fit_blocked_no_bounds";
pub(crate) const CHANNEL_UI_GRAPH_CAMERA_FIT_DEFERRED_NO_METADATA: &str =
    "runtime.ui.graph.camera_fit_deferred_no_metadata";
pub(crate) const CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT: &str =
    "runtime.ui.graph.selection_ambiguous_hit";
pub(crate) const CHANNEL_UI_GRAPH_WHEEL_ZOOM_NOT_CAPTURED: &str =
    "runtime.ui.graph.wheel_zoom_not_captured";
pub(crate) const CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED_NO_METADATA: &str =
    "runtime.ui.graph.keyboard_zoom_blocked_no_metadata";
pub(crate) const CHANNEL_UI_GRAPH_CAMERA_ZOOM_DEFERRED_NO_METADATA: &str =
    "runtime.ui.graph.camera_zoom_deferred_no_metadata";
pub(crate) const CHANNEL_UI_GRAPH_WHEEL_ZOOM_DEFERRED_NO_METADATA: &str =
    "runtime.ui.graph.wheel_zoom_deferred_no_metadata";
pub(crate) const CHANNEL_UI_GRAPH_LASSO_BLOCKED_NO_STATE: &str =
    "runtime.ui.graph.lasso_blocked_no_state";
pub(crate) const CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE: &str =
    "runtime.ui.graph.event_blocked_no_state";
pub(crate) const CHANNEL_UI_GRAPH_LAYOUT_SYNC_BLOCKED_NO_STATE: &str =
    "runtime.ui.graph.layout_sync_blocked_no_state";
pub(crate) const CHANNEL_UI_GRAPH_WHEEL_ZOOM_BLOCKED_INVALID_FACTOR: &str =
    "runtime.ui.graph.wheel_zoom_blocked_invalid_factor";
pub(crate) const CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW: &str =
    "runtime.ui.graph.camera_command_blocked_missing_target_view";
pub(crate) const CHANNEL_RUNTIME_CACHE_HIT: &str = "runtime.cache.hit";
pub(crate) const CHANNEL_RUNTIME_CACHE_MISS: &str = "runtime.cache.miss";
pub(crate) const CHANNEL_RUNTIME_CACHE_INSERT: &str = "runtime.cache.insert";
pub(crate) const CHANNEL_RUNTIME_CACHE_EVICTION: &str = "runtime.cache.eviction";
pub(crate) const CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_FIT_LOCK: &str =
    "runtime.ui.graph.keyboard_pan_blocked_fit_lock";
pub(crate) const CHANNEL_UI_GRAPH_KEYBOARD_PAN_BLOCKED_INACTIVE_VIEW: &str =
    "runtime.ui.graph.keyboard_pan_blocked_inactive_view";
pub(crate) const CHANNEL_VERSE_PREINIT_CALL: &str = "verse.preinit.call";
pub(crate) const CHANNEL_VERSE_SYNC_UNIT_SENT: &str = "verse.sync.unit_sent";
pub(crate) const CHANNEL_VERSE_SYNC_UNIT_RECEIVED: &str = "verse.sync.unit_received";
pub(crate) const CHANNEL_VERSE_SYNC_INTENT_APPLIED: &str = "verse.sync.intent_applied";
pub(crate) const CHANNEL_VERSE_SYNC_ACCESS_DENIED: &str = "verse.sync.access_denied";
pub(crate) const CHANNEL_VERSE_SYNC_CONNECTION_REJECTED: &str = "verse.sync.connection_rejected";
pub(crate) const CHANNEL_VERSE_SYNC_IDENTITY_GENERATED: &str = "verse.sync.identity_generated";
pub(crate) const CHANNEL_VERSE_SYNC_CONFLICT_DETECTED: &str = "verse.sync.conflict_detected";
pub(crate) const CHANNEL_VERSE_SYNC_CONFLICT_RESOLVED: &str = "verse.sync.conflict_resolved";
pub(crate) const CHANNEL_NOSTR_CAPABILITY_DENIED: &str = "mod.nostrcore.capability_denied";
pub(crate) const CHANNEL_NOSTR_SIGN_REQUEST_DENIED: &str = "mod.nostrcore.sign_request_denied";
pub(crate) const CHANNEL_NOSTR_RELAY_PUBLISH_FAILED: &str = "mod.nostrcore.relay_publish_failed";
pub(crate) const CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED: &str =
    "mod.nostrcore.relay_subscription_failed";
pub(crate) const CHANNEL_NOSTR_INTENT_REJECTED: &str = "mod.nostrcore.intent_rejected";
pub(crate) const CHANNEL_NOSTR_SECURITY_VIOLATION: &str = "mod.nostrcore.security_violation";
pub(crate) const CHANNEL_COMPOSITOR_GL_STATE_VIOLATION: &str = "compositor.gl_state_violation";
pub(crate) const CHANNEL_COMPOSITOR_CONTENT_PASS_REGISTERED: &str =
    "compositor.content_pass_registered";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_PASS_REGISTERED: &str =
    "compositor.overlay_pass_registered";
pub(crate) const CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION: &str =
    "compositor.pass_order_violation";
pub(crate) const CHANNEL_COMPOSITOR_INVALID_TILE_RECT: &str = "compositor.invalid_tile_rect";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS: &str = "diagnostics.compositor_chaos";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS: &str =
    "diagnostics.compositor_chaos.pass";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL: &str =
    "diagnostics.compositor_chaos.fail";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE: &str =
    "diagnostics.compositor_bridge_probe";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME: &str =
    "diagnostics.compositor_bridge_probe.failed_frame";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE: &str =
    "diagnostics.compositor_bridge_probe.callback_us_sample";
pub(crate) const CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE: &str =
    "diagnostics.compositor_bridge_probe.presentation_us_sample";
pub(crate) const CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED: &str =
    "compositor.focus_activation.deferred";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE: &str =
    "compositor.overlay.style.rect_stroke";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY: &str =
    "compositor.overlay.style.chrome_only";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE: &str =
    "compositor.overlay.mode.composited_texture";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY: &str =
    "compositor.overlay.mode.native_overlay";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI: &str =
    "compositor.overlay.mode.embedded_egui";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER: &str =
    "compositor.overlay.mode.placeholder";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_INTERACTION_MENU: &str =
    "compositor.overlay.native.suppressed.interaction_menu";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_HELP_PANEL: &str =
    "compositor.overlay.native.suppressed.help_panel";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_RADIAL_MENU: &str =
    "compositor.overlay.native.suppressed.radial_menu";
pub(crate) const CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED: &str =
    "compositor.replay.sample_recorded";
pub(crate) const CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED: &str =
    "compositor.replay.artifact_recorded";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED: &str =
    "compositor.differential.content_composed";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED: &str =
    "compositor.differential.content_skipped";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE: &str =
    "compositor.differential.fallback_no_prior_signature";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED: &str =
    "compositor.differential.fallback_signature_changed";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE: &str =
    "compositor.differential.skip_rate_basis_points";
pub(crate) const CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT: &str =
    "compositor.content.culled_offviewport";
pub(crate) const CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE: &str =
    "compositor.degradation.gpu_pressure";
pub(crate) const CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE: &str =
    "compositor.degradation.placeholder_mode";
pub(crate) const CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT: &str =
    "compositor.resource_reuse.context_hit";
pub(crate) const CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS: &str =
    "compositor.resource_reuse.context_miss";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE: &str =
    "compositor.overlay.batch_size_sample";
pub(crate) const CHANNEL_UX_DISPATCH_STARTED: &str = "ux:dispatch_started";
pub(crate) const CHANNEL_UX_DISPATCH_PHASE: &str = "ux:dispatch_phase";
pub(crate) const CHANNEL_UX_DISPATCH_CONSUMED: &str = "ux:dispatch_consumed";
pub(crate) const CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED: &str = "ux:dispatch_default_prevented";
pub(crate) const CHANNEL_UX_NAVIGATION_TRANSITION: &str = "ux:navigation_transition";
pub(crate) const CHANNEL_UX_NAVIGATION_VIOLATION: &str = "ux:navigation_violation";
pub(crate) const CHANNEL_UX_STRUCTURAL_VIOLATION: &str = "ux:structural_violation";
pub(crate) const CHANNEL_UX_CONTRACT_WARNING: &str = "ux:contract_warning";
pub(crate) const CHANNEL_UX_TREE_BUILD: &str = "ux:tree_build";
pub(crate) const CHANNEL_UX_OPEN_DECISION_PATH: &str = "ux:open_decision_path";
pub(crate) const CHANNEL_UX_OPEN_DECISION_REASON: &str = "ux:open_decision_reason";
pub(crate) const CHANNEL_UX_RADIAL_OVERFLOW: &str = "ux:radial_overflow";
pub(crate) const CHANNEL_UX_RADIAL_LAYOUT: &str = "ux:radial_layout";
pub(crate) const CHANNEL_UX_RADIAL_LABEL_COLLISION: &str = "ux:radial_label_collision";
pub(crate) const CHANNEL_UX_RADIAL_MODE_FALLBACK: &str = "ux:radial_mode_fallback";
pub(crate) const CHANNEL_UX_TREE_SNAPSHOT_BUILT: &str = "ux:tree_snapshot_built";
pub(crate) const CHANNEL_UX_PROBE_REGISTERED: &str = "ux:probe_registered";
pub(crate) const CHANNEL_UX_PROBE_DISABLED: &str = "ux:probe_disabled";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED: &str =
    "register.signal_routing.published";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED: &str =
    "register.signal_routing.unrouted";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_OBSERVER_FAILED: &str =
    "register.signal_routing.observer_failed";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_MOD_WORKFLOW_ROUTED: &str =
    "register.signal_routing.mod_workflow_routed";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_SUBSYSTEM_HEALTH_PROPAGATED: &str =
    "register.signal_routing.subsystem_health_propagated";

static REGISTRY_RUNTIME: OnceLock<RegistryRuntime> = OnceLock::new();

fn runtime() -> &'static RegistryRuntime {
    REGISTRY_RUNTIME.get_or_init(RegistryRuntime::new_with_mods)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Phase0NavigationDecision {
    pub(crate) normalized_url: ServoUrl,
    pub(crate) protocol: ProtocolResolution,
    pub(crate) viewer: ViewerSelection,
}

#[derive(Default)]
pub(crate) struct RegistryRuntime {
    action: ActionRegistry,
    #[allow(dead_code)]
    pub(crate) diagnostics: DiagnosticsRegistry,
    #[allow(dead_code)]
    signal_routing: SignalRoutingLayer,
    #[allow(dead_code)]
    identity: IdentityRegistry,
    input: Mutex<InputRegistry>,
    lens: LensRegistry,
    #[allow(dead_code)]
    nostr_core: NostrCoreRegistry,
    protocol: ProtocolRegistry,
    #[allow(dead_code)]
    renderer: Mutex<RendererRegistry>,
    viewer: ViewerRegistry,
    pub(crate) knowledge: KnowledgeRegistry,
}

#[allow(dead_code)]
pub(crate) fn phase3_sign_identity_payload(identity_id: &str, payload: &[u8]) -> Option<String> {
    runtime().sign_identity_payload(identity_id, payload)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_sign_event(
    persona: &str,
    unsigned: &NostrUnsignedEvent,
) -> Result<NostrSignedEvent, NostrCoreError> {
    runtime().nostr_core.sign_event(persona, unsigned)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_use_local_signer() {
    runtime().nostr_core.use_local_signer();
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_use_nip46_signer(
    relay_url: &str,
    signer_pubkey: &str,
) -> Result<(), NostrCoreError> {
    runtime()
        .nostr_core
        .use_nip46_signer(relay_url, signer_pubkey)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_subscribe(
    requested_id: Option<&str>,
    filters: NostrFilterSet,
) -> Result<NostrSubscriptionHandle, NostrCoreError> {
    runtime()
        .nostr_core
        .relay_subscribe("runtime:core", requested_id, filters)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_subscribe_for_caller(
    caller_id: &str,
    requested_id: Option<&str>,
    filters: NostrFilterSet,
) -> Result<NostrSubscriptionHandle, NostrCoreError> {
    runtime()
        .nostr_core
        .relay_subscribe(caller_id, requested_id, filters)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_unsubscribe(handle: &NostrSubscriptionHandle) -> bool {
    runtime()
        .nostr_core
        .relay_unsubscribe("runtime:core", handle)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_unsubscribe_for_caller(
    caller_id: &str,
    handle: &NostrSubscriptionHandle,
) -> bool {
    runtime().nostr_core.relay_unsubscribe(caller_id, handle)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_publish(
    signed: &NostrSignedEvent,
) -> Result<NostrPublishReceipt, NostrCoreError> {
    runtime().nostr_core.relay_publish("runtime:core", signed)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_publish_for_caller(
    caller_id: &str,
    signed: &NostrSignedEvent,
) -> Result<NostrPublishReceipt, NostrCoreError> {
    runtime().nostr_core.relay_publish(caller_id, signed)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_publish_to_relays(
    signed: &NostrSignedEvent,
    relay_urls: &[String],
) -> Result<NostrPublishReceipt, NostrCoreError> {
    runtime()
        .nostr_core
        .relay_publish_to_relays("runtime:core", signed, relay_urls)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_relay_publish_to_relays_for_caller(
    caller_id: &str,
    signed: &NostrSignedEvent,
    relay_urls: &[String],
) -> Result<NostrPublishReceipt, NostrCoreError> {
    runtime()
        .nostr_core
        .relay_publish_to_relays(caller_id, signed, relay_urls)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_report_intent_rejected(byte_len: usize) {
    runtime().nostr_core.report_intent_rejected(byte_len);
}

pub(crate) fn phase1_renderer_attachment_for_pane(pane_id: PaneId) -> Option<PaneAttachment> {
    runtime().renderer_attachment_for_pane(pane_id)
}

pub(crate) fn phase1_pane_for_renderer(renderer_id: RendererId) -> Option<PaneId> {
    runtime().pane_for_renderer(renderer_id)
}

pub(crate) fn phase1_attach_renderer(
    pane_id: PaneId,
    renderer_id: RendererId,
    node_key: Option<NodeKey>,
) -> Result<(), RendererRegistryError> {
    runtime().accept_renderer_attachment(pane_id, renderer_id, node_key)
}

pub(crate) fn phase1_detach_renderer(renderer_id: RendererId) -> Option<PaneAttachment> {
    runtime().detach_renderer_attachment(renderer_id)
}

impl RegistryRuntime {
    fn build_provider_wired_registries(
        protocol_providers: &ProtocolHandlerProviders,
        viewer_providers: &ViewerHandlerProviders,
    ) -> (ProtocolRegistry, ViewerRegistry) {
        let mut protocol_registry = ProtocolRegistry::default();
        let mut protocol_contract_registry = ProtocolContractRegistry::core_seed();
        protocol_providers.apply_all(&mut protocol_contract_registry);
        for scheme in protocol_contract_registry.scheme_ids() {
            protocol_registry.register_scheme(&scheme);
        }

        let mut viewer_registry = ViewerRegistry::default();
        viewer_providers.apply_all(&mut viewer_registry);

        (protocol_registry, viewer_registry)
    }

    #[cfg(test)]
    fn new_with_provider_registries_for_tests(
        protocol_providers: ProtocolHandlerProviders,
        viewer_providers: ViewerHandlerProviders,
    ) -> Self {
        let (protocol_registry, viewer_registry) =
            Self::build_provider_wired_registries(&protocol_providers, &viewer_providers);
        Self {
            action: ActionRegistry::default(),
            diagnostics: DiagnosticsRegistry::default(),
            signal_routing: SignalRoutingLayer::default(),
            identity: IdentityRegistry::default(),
            input: Mutex::new(InputRegistry::default()),
            lens: LensRegistry::default(),
            nostr_core: NostrCoreRegistry::default(),
            protocol: protocol_registry,
            renderer: Mutex::new(RendererRegistry::default()),
            viewer: viewer_registry,
            knowledge: KnowledgeRegistry::default(),
        }
    }

    /// Create a new RegistryRuntime with mods discovered and their handlers registered.
    /// This is the standard way to initialize registries during app startup (Phase 2.4).
    ///
    /// Provider wiring for protocol/viewer paths is applied into the runtime
    /// registries returned by this constructor.
    #[allow(dead_code)]
    pub(crate) fn new_with_mods() -> Self {
        // Discover and resolve mod dependencies
        let mut mod_registry = ModRegistry::new();
        if let Err(e) = mod_registry.resolve_dependencies() {
            log::error!(
                "Failed to resolve mod dependencies: {:?}. Using core seed only.",
                e
            );
        }
        let _loaded_mods = mod_registry.load_all();

        // Wire up handler providers from active mods.
        let mut protocol_providers = ProtocolHandlerProviders::new();
        let mut viewer_providers = ViewerHandlerProviders::new();

        // Register handlers from active mods
        if mod_registry.get_status("mod:verso").is_some() {
            crate::mods::verso::register_protocol_handlers(&mut protocol_providers);
            crate::mods::verso::register_viewer_handlers(&mut viewer_providers);
            log::debug!("registries: verso mod handlers registered into provider registries");
        }

        if mod_registry.get_status("mod:verse").is_some()
            || mod_registry.get_status("verse").is_some()
        {
            crate::mods::verse::register_protocol_handlers(&mut protocol_providers);
            log::debug!("registries: verse mod handlers registered into provider registries");
        }

        let (protocol_registry, viewer_registry) =
            Self::build_provider_wired_registries(&protocol_providers, &viewer_providers);

        // Create the RegistryRuntime with provider-wired registries.
        Self {
            action: ActionRegistry::default(),
            diagnostics: DiagnosticsRegistry::default(),
            signal_routing: SignalRoutingLayer::default(),
            identity: IdentityRegistry::default(),
            input: Mutex::new(InputRegistry::default()),
            lens: LensRegistry::default(),
            nostr_core: NostrCoreRegistry::default(),
            protocol: protocol_registry,
            renderer: Mutex::new(RendererRegistry::default()),
            viewer: viewer_registry,
            knowledge: KnowledgeRegistry::default(),
        }
    }

    fn accept_renderer_attachment(
        &self,
        pane_id: PaneId,
        renderer_id: RendererId,
        node_key: Option<NodeKey>,
    ) -> Result<(), RendererRegistryError> {
        let result = self
            .renderer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accept(pane_id, renderer_id, node_key);
        if result.is_ok() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_RENDERER_ATTACH,
                byte_len: 1,
            });
        }
        result
    }

    fn detach_renderer_attachment(&self, renderer_id: RendererId) -> Option<PaneAttachment> {
        let result = self
            .renderer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .detach(renderer_id);
        if result.is_some() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_RENDERER_DETACH,
                byte_len: 1,
            });
        }
        result
    }

    fn renderer_attachment_for_pane(&self, pane_id: PaneId) -> Option<PaneAttachment> {
        self.renderer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .renderer_for_pane(&pane_id)
            .cloned()
    }

    fn pane_for_renderer(&self, renderer_id: RendererId) -> Option<PaneId> {
        self.renderer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pane_for_renderer(&renderer_id)
    }

    pub(crate) fn observe_navigation_url_with_control(
        &self,
        uri: &str,
        mime_hint: Option<&str>,
        control: ProtocolResolveControl,
    ) -> Option<(ProtocolResolution, ViewerSelection)> {
        debug_assert!(!diagnostics::phase0_required_channels().is_empty());
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_PROTOCOL_RESOLVE_STARTED,
            byte_len: uri.len(),
        });
        let protocol = match self.protocol.resolve_with_control(uri, control) {
            ProtocolResolveOutcome::Resolved(resolution) => resolution,
            ProtocolResolveOutcome::Cancelled => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_PROTOCOL_RESOLVE_FAILED,
                    latency_us: 1,
                });
                return None;
            }
        };

        if protocol.supported {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_PROTOCOL_RESOLVE_SUCCEEDED,
                latency_us: 1,
            });
        } else {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_PROTOCOL_RESOLVE_FAILED,
                latency_us: 1,
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_PROTOCOL_RESOLVE_FALLBACK_USED,
                byte_len: protocol.matched_scheme.len(),
            });
        }

        let effective_mime_hint = mime_hint.or(protocol.inferred_mime_hint.as_deref());

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_VIEWER_SELECT_STARTED,
            byte_len: effective_mime_hint.unwrap_or(uri).len(),
        });
        let viewer = self.viewer.select_for_uri(uri, effective_mime_hint);
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_VIEWER_SELECT_SUCCEEDED,
            latency_us: 1,
        });

        self.publish_signal(SignalEnvelope::new(
            SignalKind::NavigationResolved {
                uri: uri.to_string(),
                viewer_id: viewer.viewer_id.to_string(),
            },
            SignalSource::RegistryRuntime,
            None,
        ));

        emit_viewer_capability_diagnostics(&viewer);
        if viewer.fallback_used {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_VIEWER_FALLBACK_USED,
                byte_len: viewer.viewer_id.len(),
            });
        }

        Some((protocol, viewer))
    }

    fn resolve_input_binding_resolution(
        &self,
        resolution: input::InputBindingResolution,
    ) -> Option<String> {
        if resolution.conflicted {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_INPUT_BINDING_CONFLICT,
                byte_len: resolution.binding_label.len(),
            });
            return None;
        }

        if resolution.matched {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_INPUT_BINDING_RESOLVED,
                byte_len: resolution.action_id.as_deref().unwrap_or_default().len(),
            });
            return resolution.action_id;
        }

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_INPUT_BINDING_MISSING,
            byte_len: resolution.binding_label.len(),
        });
        None
    }

    pub(crate) fn resolve_input_binding(&self, binding_id: &str) -> bool {
        let resolution = self
            .input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .resolve_binding_id(binding_id);
        self.resolve_input_binding_resolution(resolution)
            .is_some()
    }

    pub(crate) fn resolve_typed_input_action_id(
        &self,
        binding: &InputBinding,
        context: InputContext,
    ) -> Option<String> {
        let resolution = self
            .input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .resolve(binding, context);
        self.resolve_input_binding_resolution(resolution)
    }

    pub(crate) fn apply_input_binding_remaps(
        &self,
        remaps: &[InputBindingRemap],
    ) -> Result<(), InputRemapConflict> {
        let next_registry = InputRegistry::with_remaps(remaps)?;
        *self
            .input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next_registry;

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_INPUT_BINDING_REBOUND,
            byte_len: remaps.len(),
        });

        Ok(())
    }

    pub(crate) fn reset_input_binding_remaps(&self) {
        *self
            .input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = InputRegistry::default();
    }

    pub(crate) fn sign_identity_payload(
        &self,
        identity_id: &str,
        payload: &[u8],
    ) -> Option<String> {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_IDENTITY_SIGN_STARTED,
            byte_len: identity_id.len().saturating_add(payload.len()),
        });

        let result = self.identity.sign(identity_id, payload);
        if result.succeeded {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_IDENTITY_SIGN_SUCCEEDED,
                latency_us: 1,
            });
        } else {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_IDENTITY_SIGN_FAILED,
                latency_us: 1,
            });
        }

        if !result.resolution.key_available {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_IDENTITY_KEY_UNAVAILABLE,
                byte_len: result.resolution.resolved_id.len(),
            });
        }

        result.signature
    }

    pub(crate) fn subscribe_signal(
        &self,
        topic: SignalTopic,
        callback: impl Fn(&SignalEnvelope) -> Result<(), String> + Send + Sync + 'static,
    ) -> ObserverId {
        self.signal_routing.subscribe(topic, callback)
    }

    pub(crate) fn unsubscribe_signal(&self, topic: SignalTopic, observer_id: ObserverId) -> bool {
        self.signal_routing.unsubscribe(topic, observer_id)
    }

    #[cfg(test)]
    fn signal_routing_diagnostics(&self) -> signal_routing::SignalRoutingDiagnostics {
        self.signal_routing.diagnostics_snapshot()
    }

    fn publish_signal(&self, envelope: SignalEnvelope) {
        let report = self.signal_routing.publish(envelope);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED,
            byte_len: report.observers_notified,
        });

        if report.observers_notified == 0 {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED,
                byte_len: 0,
            });
        }

        if report.observer_failures > 0 {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_OBSERVER_FAILED,
                byte_len: report.observer_failures,
            });
        }
    }

    pub(crate) fn route_mod_lifecycle_event(&self, mod_id: &str, activated: bool) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::ModLifecycleChanged {
                mod_id: mod_id.to_string(),
                activated,
            },
            SignalSource::ControlPanel,
            None,
        ));

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_MOD_WORKFLOW_ROUTED,
            byte_len: mod_id.len(),
        });
    }

    pub(crate) fn propagate_subsystem_health_memory_pressure(
        &self,
        level: MemoryPressureLevel,
        available_mib: u64,
        total_mib: u64,
    ) {
        let level_name = match level {
            MemoryPressureLevel::Unknown => "unknown",
            MemoryPressureLevel::Normal => "normal",
            MemoryPressureLevel::Warning => "warning",
            MemoryPressureLevel::Critical => "critical",
        };

        self.publish_signal(SignalEnvelope::new(
            SignalKind::SubsystemHealthMemoryPressure {
                level: level_name.to_string(),
                available_mib,
                total_mib,
            },
            SignalSource::ControlPanel,
            None,
        ));

        let byte_len = (available_mib as usize).saturating_add(total_mib as usize);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_SUBSYSTEM_HEALTH_PROPAGATED,
            byte_len,
        });
    }
}

pub(crate) fn phase2_resolve_toolbar_submit_binding() -> bool {
    phase2_resolve_input_binding(INPUT_BINDING_TOOLBAR_SUBMIT)
}

pub(crate) fn phase3_route_mod_lifecycle_event(mod_id: &str, activated: bool) {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().route_mod_lifecycle_event(mod_id, activated);
}

pub(crate) fn phase3_propagate_subsystem_health_memory_pressure(
    level: MemoryPressureLevel,
    available_mib: u64,
    total_mib: u64,
) {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().propagate_subsystem_health_memory_pressure(level, available_mib, total_mib);
}

pub(crate) fn phase2_resolve_input_binding(binding_id: &str) -> bool {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());
    runtime().resolve_input_binding(binding_id)
}

pub(crate) fn phase2_resolve_typed_input_action_id(
    binding: &InputBinding,
    context: InputContext,
) -> Option<String> {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());
    runtime().resolve_typed_input_action_id(binding, context)
}

pub(crate) fn phase2_apply_input_binding_remaps(
    remaps: &[InputBindingRemap],
) -> Result<(), InputRemapConflict> {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());
    runtime().apply_input_binding_remaps(remaps)
}

pub(crate) fn phase2_reset_input_binding_remaps() {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());
    runtime().reset_input_binding_remaps();
}

pub(crate) fn phase2_resolve_lens(lens_id: &str) -> crate::app::LensConfig {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    let runtime = runtime();
    let resolution = runtime.lens.resolve(lens_id);
    log::debug!(
        "registry lens resolve requested='{}' resolved='{}' matched={} fallback={}",
        resolution.requested_id,
        resolution.resolved_id,
        resolution.matched,
        resolution.fallback_used
    );

    if resolution.matched {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_LENS_RESOLVE_SUCCEEDED,
            latency_us: 1,
        });
    } else {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_LENS_RESOLVE_FAILED,
            latency_us: 1,
        });
    }

    if resolution.fallback_used {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_LENS_FALLBACK_USED,
            byte_len: resolution.resolved_id.len(),
        });
    }

    crate::app::LensConfig {
        name: resolution.definition.display_name,
        lens_id: Some(resolution.resolved_id),
        physics: resolution.definition.physics,
        layout: resolution.definition.layout,
        theme: resolution.definition.theme,
        filters: resolution.definition.filters,
    }
}

fn emit_viewer_capability_diagnostics(viewer: &ViewerSelection) {
    for (level, reason) in [
        (
            &viewer.capabilities.accessibility.level,
            viewer.capabilities.accessibility.reason.as_deref(),
        ),
        (
            &viewer.capabilities.security.level,
            viewer.capabilities.security.reason.as_deref(),
        ),
        (
            &viewer.capabilities.storage.level,
            viewer.capabilities.storage.reason.as_deref(),
        ),
        (
            &viewer.capabilities.history.level,
            viewer.capabilities.history.reason.as_deref(),
        ),
    ] {
        match level {
            ConformanceLevel::Full => {}
            ConformanceLevel::Partial => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_VIEWER_CAPABILITY_PARTIAL,
                    byte_len: reason.unwrap_or_default().len(),
                });
            }
            ConformanceLevel::None => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_VIEWER_CAPABILITY_NONE,
                    byte_len: reason.unwrap_or_default().len(),
                });
            }
        }
    }
}

fn emit_surface_conformance_diagnostics(
    accessibility: ConformanceLevel,
    security: ConformanceLevel,
    storage: ConformanceLevel,
    history: ConformanceLevel,
) {
    for level in [accessibility, security, storage, history] {
        match level {
            ConformanceLevel::Full => {}
            ConformanceLevel::Partial => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_SURFACE_CONFORMANCE_PARTIAL,
                    byte_len: 1,
                });
            }
            ConformanceLevel::None => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_SURFACE_CONFORMANCE_NONE,
                    byte_len: 1,
                });
            }
        }
    }
}

pub(crate) fn phase2_execute_omnibox_node_search_action(
    app: &GraphBrowserApp,
    query: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_ACTION_EXECUTE_STARTED,
        byte_len: query.len(),
    });

    let execution = runtime().action.execute(
        ACTION_OMNIBOX_NODE_SEARCH,
        app,
        ActionPayload::OmniboxNodeSearch {
            query: query.to_string(),
        },
    );
    let succeeded = execution.succeeded();
    let intent_len = execution.intent_len();

    log::debug!(
        "registry action '{}' executed for omnibox query '{}'; succeeded={} intents={}",
        ACTION_OMNIBOX_NODE_SEARCH,
        query,
        succeeded,
        intent_len
    );

    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        latency_us: 1,
    });

    execution.into_intents()
}

pub(crate) struct Phase2GraphViewSubmitResult {
    pub(crate) open_selected_tile: bool,
    pub(crate) mutations: Vec<GraphMutation>,
}

pub(crate) struct Phase2DetailViewSubmitResult {
    pub(crate) open_selected_tile: bool,
    pub(crate) mutations: Vec<GraphMutation>,
    pub(crate) runtime_events: Vec<RuntimeEvent>,
}

fn expect_graph_mutations(intents: Vec<GraphIntent>, action_id: &str) -> Vec<GraphMutation> {
    intents
        .into_iter()
        .map(|intent| {
            intent.as_graph_mutation().unwrap_or_else(|| {
                panic!("phase-2 action '{action_id}' emitted non-mutation intent: {intent:?}")
            })
        })
        .collect()
}

fn split_detail_submit_intents(
    intents: Vec<GraphIntent>,
    action_id: &str,
) -> (Vec<GraphMutation>, Vec<RuntimeEvent>) {
    let mut mutations = Vec::new();
    let mut runtime_events = Vec::new();

    for intent in intents {
        if let Some(mutation) = intent.as_graph_mutation() {
            mutations.push(mutation);
            continue;
        }
        if let Some(runtime_event) = intent.as_runtime_event() {
            runtime_events.push(runtime_event);
            continue;
        }
        panic!("phase-2 action '{action_id}' emitted unsupported mixed intent: {intent:?}");
    }

    (mutations, runtime_events)
}

pub(crate) fn phase5_execute_verse_sync_now_action(app: &GraphBrowserApp) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution =
        runtime()
            .action
            .execute(ACTION_VERSE_SYNC_NOW, app, ActionPayload::VerseSyncNow);
    execution.into_intents()
}

pub(crate) fn phase5_execute_verse_pair_local_peer_action(
    app: &GraphBrowserApp,
    node_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_PAIR_DEVICE,
        app,
        ActionPayload::VersePairDevice {
            mode: PairingMode::LocalPeer {
                node_id: node_id.to_string(),
            },
        },
    );
    execution.into_intents()
}

pub(crate) fn phase5_execute_verse_pair_code_action(
    app: &GraphBrowserApp,
    code: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_PAIR_DEVICE,
        app,
        ActionPayload::VersePairDevice {
            mode: PairingMode::EnterCode {
                code: code.to_string(),
            },
        },
    );
    execution.into_intents()
}

pub(crate) fn phase5_execute_verse_share_workspace_action(
    app: &GraphBrowserApp,
    workspace_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_SHARE_WORKSPACE,
        app,
        ActionPayload::VerseShareWorkspace {
            workspace_id: workspace_id.to_string(),
        },
    );
    execution.into_intents()
}

pub(crate) fn phase5_execute_verse_forget_device_action(
    app: &GraphBrowserApp,
    node_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_FORGET_DEVICE,
        app,
        ActionPayload::VerseForgetDevice {
            node_id: node_id.to_string(),
        },
    );
    execution.into_intents()
}

#[allow(dead_code)]
pub(crate) fn register_mod_diagnostics_channel(
    mod_id: &str,
    channel_id: &str,
    schema_version: u16,
    description: Option<String>,
) -> Result<bool, diagnostics::ChannelRegistrationError> {
    let created = diagnostics::register_mod_channel_global(
        mod_id,
        channel_id,
        schema_version,
        description,
        &[diagnostics::DiagnosticsCapability::RegisterChannels],
    )?;
    if created {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_CHANNEL_REGISTERED,
            byte_len: channel_id.len(),
        });
    }
    Ok(created)
}

#[allow(dead_code)]
pub(crate) fn register_verse_diagnostics_channel(
    peer_id: &str,
    channel_id: &str,
    schema_version: u16,
    description: Option<String>,
) -> Result<bool, diagnostics::ChannelRegistrationError> {
    let created = diagnostics::register_verse_channel_global(
        peer_id,
        channel_id,
        schema_version,
        description,
        &[diagnostics::DiagnosticsCapability::RegisterChannels],
    )?;
    if created {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_CHANNEL_REGISTERED,
            byte_len: channel_id.len(),
        });
    }
    Ok(created)
}

#[allow(dead_code)]
pub(crate) fn register_diagnostics_invariant(
    invariant: diagnostics::DiagnosticsInvariant,
) -> Result<bool, diagnostics::ChannelRegistrationError> {
    diagnostics::register_invariant_global(
        invariant,
        &[diagnostics::DiagnosticsCapability::RegisterInvariants],
    )
}

#[cfg(test)]
pub(crate) fn phase3_sign_identity_payload_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    identity_id: &str,
    payload: &[u8],
) -> Option<String> {
    diagnostics_state.emit_message_sent_for_tests(
        CHANNEL_IDENTITY_SIGN_STARTED,
        identity_id.len().saturating_add(payload.len()),
    );

    let result = RegistryRuntime::default()
        .identity
        .sign(identity_id, payload);
    diagnostics_state.emit_message_received_for_tests(
        if result.succeeded {
            CHANNEL_IDENTITY_SIGN_SUCCEEDED
        } else {
            CHANNEL_IDENTITY_SIGN_FAILED
        },
        1,
    );

    if !result.resolution.key_available {
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_IDENTITY_KEY_UNAVAILABLE,
            result.resolution.resolved_id.len(),
        );
    }

    result.signature
}

#[cfg(test)]
pub(crate) fn phase2_resolve_toolbar_submit_binding_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) -> bool {
    phase2_resolve_input_binding_for_tests(diagnostics_state, INPUT_BINDING_TOOLBAR_SUBMIT)
}

#[cfg(test)]
pub(crate) fn phase2_resolve_input_binding_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    binding_id: &str,
) -> bool {
    let runtime = RegistryRuntime::default();
    let resolution = runtime
        .input
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .resolve_binding_id(binding_id);

    if resolution.conflicted {
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_INPUT_BINDING_CONFLICT,
            resolution.binding_label.len(),
        );
        return false;
    }

    if resolution.matched {
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_INPUT_BINDING_RESOLVED,
            resolution.action_id.as_deref().unwrap_or_default().len(),
        );
        return true;
    }

    diagnostics_state
        .emit_message_sent_for_tests(CHANNEL_INPUT_BINDING_MISSING, resolution.binding_label.len());
    false
}

#[cfg(test)]
pub(crate) fn phase2_resolve_lens_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    lens_id: &str,
) -> crate::app::LensConfig {
    let runtime = RegistryRuntime::default();
    let resolution = runtime.lens.resolve(lens_id);

    if resolution.matched {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_LENS_RESOLVE_SUCCEEDED, 1);
    } else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_LENS_RESOLVE_FAILED, 1);
    }

    if resolution.fallback_used {
        diagnostics_state
            .emit_message_sent_for_tests(CHANNEL_LENS_FALLBACK_USED, resolution.resolved_id.len());
    }

    crate::app::LensConfig {
        name: resolution.definition.display_name,
        lens_id: Some(resolution.resolved_id),
        physics: resolution.definition.physics,
        layout: resolution.definition.layout,
        theme: resolution.definition.theme,
        filters: resolution.definition.filters,
    }
}

pub(crate) fn phase2_execute_graph_view_submit_action(
    app: &GraphBrowserApp,
    input: &str,
) -> Phase2GraphViewSubmitResult {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_ACTION_EXECUTE_STARTED,
        byte_len: input.len(),
    });

    let execution = runtime().action.execute(
        ACTION_GRAPH_VIEW_SUBMIT,
        app,
        ActionPayload::GraphViewSubmit {
            input: input.to_string(),
        },
    );
    let succeeded = execution.succeeded();
    let intent_len = execution.intent_len();
    let intents = execution.into_intents();

    log::debug!(
        "registry action '{}' executed for graph-view submit '{}'; succeeded={} intents={}",
        ACTION_GRAPH_VIEW_SUBMIT,
        input,
        succeeded,
        intent_len
    );

    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        latency_us: 1,
    });

    let mutations = expect_graph_mutations(intents, ACTION_GRAPH_VIEW_SUBMIT);
    let open_selected_tile = succeeded && !mutations.is_empty();
    Phase2GraphViewSubmitResult {
        open_selected_tile,
        mutations,
    }
}

pub(crate) fn phase2_execute_detail_view_submit_action(
    app: &GraphBrowserApp,
    normalized_url: &str,
    focused_node: Option<crate::graph::NodeKey>,
) -> Phase2DetailViewSubmitResult {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_ACTION_EXECUTE_STARTED,
        byte_len: normalized_url.len(),
    });

    let execution = runtime().action.execute(
        ACTION_DETAIL_VIEW_SUBMIT,
        app,
        ActionPayload::DetailViewSubmit {
            normalized_url: normalized_url.to_string(),
            focused_node,
        },
    );
    let succeeded = execution.succeeded();
    let intent_len = execution.intent_len();
    let intents = execution.into_intents();

    log::debug!(
        "registry action '{}' executed for detail-view submit '{}'; succeeded={} intents={}",
        ACTION_DETAIL_VIEW_SUBMIT,
        normalized_url,
        succeeded,
        intent_len
    );

    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        latency_us: 1,
    });

    let (mutations, runtime_events) = split_detail_submit_intents(intents, ACTION_DETAIL_VIEW_SUBMIT);
    let open_selected_tile = mutations
        .iter()
        .any(|mutation| matches!(mutation, GraphMutation::CreateNodeAtUrl { .. }));
    Phase2DetailViewSubmitResult {
        open_selected_tile,
        mutations,
        runtime_events,
    }
}

fn phase0_observe_navigation_url_with_control(
    uri: &str,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<(ProtocolResolution, ViewerSelection)> {
    runtime().observe_navigation_url_with_control(uri, mime_hint, control)
}

fn apply_phase0_protocol_policy(parsed_url: ServoUrl, resolution: &ProtocolResolution) -> ServoUrl {
    if resolution.supported || !resolution.fallback_used || resolution.matched_scheme.is_empty() {
        return parsed_url;
    }

    if let Some((_, remainder)) = parsed_url.as_str().split_once(':') {
        let rewritten = format!("{}:{}", resolution.matched_scheme, remainder);
        if let Ok(rewritten_url) = ServoUrl::parse(&rewritten) {
            return rewritten_url;
        }
    }

    parsed_url
}

pub(crate) fn phase0_decide_navigation_with_control(
    parsed_url: ServoUrl,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<Phase0NavigationDecision> {
    let (protocol_resolution, viewer_selection) =
        phase0_observe_navigation_url_with_control(parsed_url.as_str(), mime_hint, control)?;
    if viewer_selection.viewer_id != "viewer:webview" {
        log::debug!(
            "registry viewer '{}' selected for {}; keeping webview path in Phase 0",
            viewer_selection.viewer_id,
            parsed_url.as_str()
        );
    }

    let normalized_url = apply_phase0_protocol_policy(parsed_url, &protocol_resolution);
    Some(Phase0NavigationDecision {
        normalized_url,
        protocol: protocol_resolution,
        viewer: viewer_selection,
    })
}

pub(crate) fn phase3_resolve_viewer_surface_profile(_viewer_id: &str) -> ViewerSurfaceResolution {
    let layout_domain = LayoutDomainRegistry::default();
    let profile_resolution = layout_domain.resolve_profile(
        crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT,
        crate::registries::domain::layout::workbench_surface::WORKBENCH_SURFACE_DEFAULT,
        VIEWER_SURFACE_DEFAULT,
    );

    let resolution = profile_resolution.viewer_surface;
    emit_surface_conformance_diagnostics(
        profile_resolution
            .canvas
            .profile
            .subsystems
            .accessibility
            .level,
        profile_resolution.canvas.profile.subsystems.security.level,
        profile_resolution.canvas.profile.subsystems.storage.level,
        profile_resolution.canvas.profile.subsystems.history.level,
    );
    emit_surface_conformance_diagnostics(
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .accessibility
            .level,
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .security
            .level,
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .storage
            .level,
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .history
            .level,
    );
    emit_surface_conformance_diagnostics(
        resolution.profile.subsystems.accessibility.level.clone(),
        resolution.profile.subsystems.security.level.clone(),
        resolution.profile.subsystems.storage.level.clone(),
        resolution.profile.subsystems.history.level.clone(),
    );
    resolution
}

#[cfg(test)]
fn phase0_observe_navigation_url_for_tests_with_control(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    uri: &str,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<(ProtocolResolution, ViewerSelection)> {
    let runtime = RegistryRuntime::default();

    diagnostics_state.emit_message_sent_for_tests(CHANNEL_PROTOCOL_RESOLVE_STARTED, uri.len());
    let protocol = match runtime.protocol.resolve_with_control(uri, control) {
        ProtocolResolveOutcome::Resolved(resolution) => resolution,
        ProtocolResolveOutcome::Cancelled => {
            diagnostics_state.emit_message_received_for_tests(CHANNEL_PROTOCOL_RESOLVE_FAILED, 1);
            return None;
        }
    };
    if protocol.supported {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_PROTOCOL_RESOLVE_SUCCEEDED, 1);
    } else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_PROTOCOL_RESOLVE_FAILED, 1);
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_PROTOCOL_RESOLVE_FALLBACK_USED,
            protocol.matched_scheme.len(),
        );
    }

    let effective_mime_hint = mime_hint.or(protocol.inferred_mime_hint.as_deref());

    diagnostics_state.emit_message_sent_for_tests(
        CHANNEL_VIEWER_SELECT_STARTED,
        effective_mime_hint.unwrap_or(uri).len(),
    );
    let viewer = runtime.viewer.select_for_uri(uri, effective_mime_hint);
    diagnostics_state.emit_message_received_for_tests(CHANNEL_VIEWER_SELECT_SUCCEEDED, 1);
    for (level, reason) in [
        (
            &viewer.capabilities.accessibility.level,
            viewer.capabilities.accessibility.reason.as_deref(),
        ),
        (
            &viewer.capabilities.security.level,
            viewer.capabilities.security.reason.as_deref(),
        ),
        (
            &viewer.capabilities.storage.level,
            viewer.capabilities.storage.reason.as_deref(),
        ),
        (
            &viewer.capabilities.history.level,
            viewer.capabilities.history.reason.as_deref(),
        ),
    ] {
        match level {
            ConformanceLevel::Full => {}
            ConformanceLevel::Partial => diagnostics_state.emit_message_sent_for_tests(
                CHANNEL_VIEWER_CAPABILITY_PARTIAL,
                reason.unwrap_or_default().len(),
            ),
            ConformanceLevel::None => diagnostics_state.emit_message_sent_for_tests(
                CHANNEL_VIEWER_CAPABILITY_NONE,
                reason.unwrap_or_default().len(),
            ),
        }
    }
    if viewer.fallback_used {
        diagnostics_state
            .emit_message_sent_for_tests(CHANNEL_VIEWER_FALLBACK_USED, viewer.viewer_id.len());
    }

    Some((protocol, viewer))
}

#[cfg(test)]
pub(crate) fn phase0_decide_navigation_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    parsed_url: ServoUrl,
    mime_hint: Option<&str>,
) -> Phase0NavigationDecision {
    phase0_decide_navigation_for_tests_with_control(
        diagnostics_state,
        parsed_url,
        mime_hint,
        ProtocolResolveControl::default(),
    )
    .expect("default protocol resolve control must remain active")
}

#[cfg(test)]
pub(crate) fn phase2_execute_omnibox_node_search_action_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    app: &GraphBrowserApp,
    query: &str,
) -> Vec<GraphIntent> {
    diagnostics_state.emit_message_sent_for_tests(CHANNEL_ACTION_EXECUTE_STARTED, query.len());

    let execution = RegistryRuntime::default().action.execute(
        ACTION_OMNIBOX_NODE_SEARCH,
        app,
        ActionPayload::OmniboxNodeSearch {
            query: query.to_string(),
        },
    );
    let succeeded = execution.succeeded();
    let intent_len = execution.intent_len();

    log::debug!(
        "registry action '{}' executed in test flow; succeeded={} intents={}",
        ACTION_OMNIBOX_NODE_SEARCH,
        succeeded,
        intent_len
    );

    diagnostics_state.emit_message_received_for_tests(
        if succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        1,
    );

    execution.into_intents()
}

#[cfg(test)]
pub(crate) fn phase2_execute_graph_view_submit_action_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    app: &GraphBrowserApp,
    input: &str,
) -> Phase2GraphViewSubmitResult {
    diagnostics_state.emit_message_sent_for_tests(CHANNEL_ACTION_EXECUTE_STARTED, input.len());

    let execution = RegistryRuntime::default().action.execute(
        ACTION_GRAPH_VIEW_SUBMIT,
        app,
        ActionPayload::GraphViewSubmit {
            input: input.to_string(),
        },
    );
    let succeeded = execution.succeeded();
    let intents = execution.into_intents();

    diagnostics_state.emit_message_received_for_tests(
        if succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        1,
    );

    let mutations = expect_graph_mutations(intents, ACTION_GRAPH_VIEW_SUBMIT);
    let open_selected_tile = succeeded && !mutations.is_empty();
    Phase2GraphViewSubmitResult {
        open_selected_tile,
        mutations,
    }
}

#[cfg(test)]
pub(crate) fn phase2_execute_detail_view_submit_action_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    app: &GraphBrowserApp,
    normalized_url: &str,
    focused_node: Option<crate::graph::NodeKey>,
) -> Phase2DetailViewSubmitResult {
    diagnostics_state
        .emit_message_sent_for_tests(CHANNEL_ACTION_EXECUTE_STARTED, normalized_url.len());

    let execution = RegistryRuntime::default().action.execute(
        ACTION_DETAIL_VIEW_SUBMIT,
        app,
        ActionPayload::DetailViewSubmit {
            normalized_url: normalized_url.to_string(),
            focused_node,
        },
    );
    let succeeded = execution.succeeded();
    let intents = execution.into_intents();

    diagnostics_state.emit_message_received_for_tests(
        if succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        1,
    );

    let (mutations, runtime_events) = split_detail_submit_intents(intents, ACTION_DETAIL_VIEW_SUBMIT);
    let open_selected_tile = mutations
        .iter()
        .any(|mutation| matches!(mutation, GraphMutation::CreateNodeAtUrl { .. }));
    Phase2DetailViewSubmitResult {
        open_selected_tile,
        mutations,
        runtime_events,
    }
}

#[cfg(test)]
pub(crate) fn phase0_decide_navigation_for_tests_with_control(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    parsed_url: ServoUrl,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<Phase0NavigationDecision> {
    let (protocol_resolution, viewer_selection) =
        phase0_observe_navigation_url_for_tests_with_control(
            diagnostics_state,
            parsed_url.as_str(),
            mime_hint,
            control,
        )?;
    if viewer_selection.viewer_id != "viewer:webview" {
        log::debug!(
            "registry viewer '{}' selected for {}; keeping webview path in Phase 0",
            viewer_selection.viewer_id,
            parsed_url.as_str()
        );
    }

    let normalized_url = apply_phase0_protocol_policy(parsed_url, &protocol_resolution);
    Some(Phase0NavigationDecision {
        normalized_url,
        protocol: protocol_resolution,
        viewer: viewer_selection,
    })
}

/// Test-harness helper: evaluate workspace access control for an inbound sync.
///
/// Returns `true` if the peer is permitted to apply the described sync (access granted),
/// or `false` if the sync should be rejected.  When rejected the
/// `verse.sync.access_denied` diagnostic channel is emitted through `diagnostics_state`.
/// No graph mutations are performed by this function.
#[cfg(test)]
pub(crate) fn phase5_check_verse_workspace_sync_access_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    peers: &[crate::mods::native::verse::TrustedPeer],
    peer_id: iroh::NodeId,
    workspace_id: &str,
    has_mutating_intents: bool,
) -> bool {
    use crate::mods::native::verse::AccessLevel;
    use crate::mods::native::verse::sync_worker::resolve_peer_grant;

    let access = resolve_peer_grant(peers, peer_id, workspace_id);

    let Some(access) = access else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_VERSE_SYNC_ACCESS_DENIED, 0);
        return false;
    };

    if access == AccessLevel::ReadOnly && has_mutating_intents {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_VERSE_SYNC_ACCESS_DENIED, 0);
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    use super::*;

    fn nostr_backend_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("nostr backend test lock poisoned")
    }

    #[test]
    fn protocol_registry_resolves_known_scheme_and_falls_back_for_unknown() {
        let registry = ProtocolRegistry::default();
        let https = registry.resolve("https://example.com");
        assert!(https.supported);
        assert!(!https.fallback_used);

        let custom = registry.resolve("foo://example.com");
        assert!(!custom.supported);
        assert!(custom.fallback_used);
        assert_eq!(custom.matched_scheme, "https");

        let graphshell = registry.resolve(
            &crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
                .to_string(),
        );
        assert!(graphshell.supported);
        assert_eq!(graphshell.matched_scheme, "verso");
        assert_eq!(
            graphshell.inferred_mime_hint.as_deref(),
            Some("application/x-graphshell-settings")
        );
    }

    #[test]
    fn viewer_registry_prefers_mime_then_extension_then_fallback() {
        let registry = ViewerRegistry::default();
        let by_mime = registry.select_for_uri("https://example.com/file.bin", Some("text/csv"));
        assert_eq!(by_mime.viewer_id, "viewer:csv");
        assert_eq!(by_mime.matched_by, "mime");

        let by_ext = registry.select_for_uri("https://example.com/readme.md", None);
        assert_eq!(by_ext.viewer_id, "viewer:markdown");
        assert_eq!(by_ext.matched_by, "extension");

        let fallback = registry.select_for_uri("https://example.com/archive.unknown", None);
        assert!(fallback.fallback_used);
        assert_eq!(fallback.viewer_id, "viewer:webview");

        let internal = registry.select_for_uri(
            &crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
                .to_string(),
            None,
        );
        assert_eq!(internal.viewer_id, "viewer:settings");
        assert_eq!(internal.matched_by, "internal");
    }

    #[test]
    fn new_with_mods_applies_provider_wiring_to_runtime_viewer_dispatch() {
        let baseline = ViewerRegistry::default()
            .select_for_uri("https://example.com/diagram.svg", Some("image/svg+xml"));
        let runtime = RegistryRuntime::new_with_mods();
        let (_, viewer) = runtime
            .observe_navigation_url_with_control(
                "https://example.com/diagram.svg",
                Some("image/svg+xml"),
                ProtocolResolveControl::default(),
            )
            .expect("default protocol resolve control should be active");

        assert!(baseline.fallback_used);
        assert_eq!(viewer.viewer_id, "viewer:webview");
        assert!(!viewer.fallback_used);
        assert_ne!(viewer.matched_by, "fallback");
    }

    #[test]
    fn provider_wired_runtime_applies_protocol_provider_dispatch() {
        let baseline = ProtocolRegistry::default().resolve("modtest://example.com/path");
        assert!(!baseline.supported);
        assert!(baseline.fallback_used);

        let mut protocol_providers = ProtocolHandlerProviders::new();
        protocol_providers.register_fn(|registry| {
            registry.register_scheme("modtest", "protocol:modtest");
        });

        let runtime = RegistryRuntime::new_with_provider_registries_for_tests(
            protocol_providers,
            ViewerHandlerProviders::new(),
        );

        let (protocol, _viewer) = runtime
            .observe_navigation_url_with_control(
                "modtest://example.com/path",
                None,
                ProtocolResolveControl::default(),
            )
            .expect("default protocol resolve control should be active");

        assert!(protocol.supported);
        assert!(!protocol.fallback_used);
        assert_eq!(protocol.matched_scheme, "modtest");
    }

    #[test]
    fn registry_runtime_navigation_signal_producer_notifies_two_observers() {
        let runtime = RegistryRuntime::default();
        let observer_a = Arc::new(AtomicUsize::new(0));
        let observer_b = Arc::new(AtomicUsize::new(0));

        {
            let observer_a = Arc::clone(&observer_a);
            runtime.subscribe_signal(SignalTopic::Navigation, move |_signal| {
                observer_a.fetch_add(1, Ordering::Relaxed);
                Ok(())
            });
        }

        {
            let observer_b = Arc::clone(&observer_b);
            runtime.subscribe_signal(SignalTopic::Navigation, move |_signal| {
                observer_b.fetch_add(1, Ordering::Relaxed);
                Ok(())
            });
        }

        let result = runtime.observe_navigation_url_with_control(
            "https://example.com",
            None,
            ProtocolResolveControl::default(),
        );
        assert!(result.is_some());
        assert_eq!(observer_a.load(Ordering::Relaxed), 1);
        assert_eq!(observer_b.load(Ordering::Relaxed), 1);

        let diagnostics = runtime.signal_routing_diagnostics();
        assert_eq!(diagnostics.published_signals, 1);
        assert_eq!(diagnostics.routed_deliveries, 2);
        assert_eq!(diagnostics.unrouted_signals, 0);
        assert_eq!(diagnostics.observer_failures, 0);
    }

    #[test]
    fn registry_runtime_signal_unsubscribe_stops_navigation_delivery() {
        let runtime = RegistryRuntime::default();
        let observer_count = Arc::new(AtomicUsize::new(0));
        let observer_id = {
            let observer_count = Arc::clone(&observer_count);
            runtime.subscribe_signal(SignalTopic::Navigation, move |_signal| {
                observer_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            })
        };

        assert!(runtime.unsubscribe_signal(SignalTopic::Navigation, observer_id));

        let result = runtime.observe_navigation_url_with_control(
            "https://example.com/after-unsubscribe",
            None,
            ProtocolResolveControl::default(),
        );
        assert!(result.is_some());
        assert_eq!(observer_count.load(Ordering::Relaxed), 0);

        let diagnostics = runtime.signal_routing_diagnostics();
        assert_eq!(diagnostics.published_signals, 1);
        assert_eq!(diagnostics.routed_deliveries, 0);
        assert_eq!(diagnostics.unrouted_signals, 0);
    }

    #[test]
    fn phase2_input_binding_path_matches_registry_runtime_dispatch_api() {
        let via_phase_api = phase2_resolve_input_binding(INPUT_BINDING_TOOLBAR_SUBMIT);
        let via_runtime_api = runtime().resolve_input_binding(INPUT_BINDING_TOOLBAR_SUBMIT);
        assert_eq!(via_phase_api, via_runtime_api);
    }

    #[test]
    fn phase3_identity_sign_path_matches_registry_runtime_dispatch_api() {
        let identity_id = "identity:missing";
        let payload = b"payload";

        let via_phase_api = phase3_sign_identity_payload(identity_id, payload);
        let via_runtime_api = runtime().sign_identity_payload(identity_id, payload);

        assert_eq!(via_phase_api, via_runtime_api);
    }

    #[test]
    fn phase3_mod_workflow_path_routes_through_lifecycle_signal_observers() {
        let runtime = RegistryRuntime::default();
        let observer_a = Arc::new(AtomicUsize::new(0));
        let observer_b = Arc::new(AtomicUsize::new(0));

        {
            let observer_a = Arc::clone(&observer_a);
            runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if matches!(
                    signal.kind,
                    SignalKind::ModLifecycleChanged {
                        activated: true,
                        ..
                    }
                ) {
                    observer_a.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        {
            let observer_b = Arc::clone(&observer_b);
            runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if let SignalKind::ModLifecycleChanged { mod_id, activated } = &signal.kind
                    && *activated
                    && mod_id == "mod:test"
                {
                    observer_b.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        runtime.route_mod_lifecycle_event("mod:test", true);

        assert_eq!(observer_a.load(Ordering::Relaxed), 1);
        assert_eq!(observer_b.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn phase3_subsystem_health_memory_pressure_routes_through_lifecycle_signals() {
        let runtime = RegistryRuntime::default();
        let seen_warning = Arc::new(AtomicUsize::new(0));

        {
            let seen_warning = Arc::clone(&seen_warning);
            runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if let SignalKind::SubsystemHealthMemoryPressure {
                    level,
                    available_mib,
                    total_mib,
                } = &signal.kind
                    && level == "warning"
                    && *available_mib == 512
                    && *total_mib == 4096
                {
                    seen_warning.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        runtime.propagate_subsystem_health_memory_pressure(MemoryPressureLevel::Warning, 512, 4096);

        assert_eq!(seen_warning.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn phase0_dispatch_uses_provider_wired_runtime_singleton() {
        let baseline = ViewerRegistry::default()
            .select_for_uri("https://example.com/diagram.svg", Some("image/svg+xml"));
        let parsed = ServoUrl::parse("https://example.com/diagram.svg").expect("url should parse");

        let decision = phase0_decide_navigation_with_control(
            parsed,
            Some("image/svg+xml"),
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should be active");

        assert!(baseline.fallback_used);
        assert_eq!(decision.viewer.viewer_id, "viewer:webview");
        assert!(!decision.viewer.fallback_used);
        assert_ne!(decision.viewer.matched_by, "fallback");
    }

    #[test]
    fn diagnostics_registry_declares_phase0_channels_with_versions() {
        let channels = diagnostics::phase0_required_channels();
        assert!(channels.len() >= 7);
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_PROTOCOL_RESOLVE_STARTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VIEWER_SELECT_SUCCEEDED)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase2_action_channels_with_versions() {
        let channels = diagnostics::phase2_required_channels();
        assert!(channels.len() >= 3);
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_ACTION_EXECUTE_STARTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_ACTION_EXECUTE_SUCCEEDED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_ACTION_EXECUTE_FAILED)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase2_input_channels_with_versions() {
        let channels = diagnostics::phase2_required_channels();
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INPUT_BINDING_RESOLVED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INPUT_BINDING_MISSING)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INPUT_BINDING_CONFLICT)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase3_identity_channels_with_versions() {
        let channels = diagnostics::phase3_required_channels();
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_SIGN_STARTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_SIGN_SUCCEEDED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_SIGN_FAILED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_KEY_UNAVAILABLE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_GL_STATE_VIOLATION)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase5_verse_channels_with_versions() {
        let channels = diagnostics::phase5_required_channels();
        assert!(channels.len() >= 6);
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_UNIT_SENT)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_UNIT_RECEIVED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_INTENT_APPLIED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_ACCESS_DENIED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_CONNECTION_REJECTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_IDENTITY_GENERATED)
        );
    }

    #[test]
    fn diagnostics_registry_exposes_phase5_invariants_via_api_surface() {
        let expected_ids = diagnostics::phase5_required_invariant_ids();
        assert_eq!(expected_ids.len(), 2);

        let invariants = diagnostics::list_invariants_snapshot();
        for invariant_id in expected_ids {
            assert!(
                invariants
                    .iter()
                    .any(|entry| entry.invariant_id == *invariant_id),
                "missing required phase5 invariant: {invariant_id}"
            );
        }
    }

    #[test]
    fn phase3_identity_registry_signs_default_payload() {
        let signature = phase3_sign_identity_payload("identity:default", b"payload");
        assert!(
            signature
                .as_deref()
                .is_some_and(|sig| sig.starts_with("sig:"))
        );
    }

    #[test]
    fn phase2_input_registry_resolves_toolbar_submit_binding() {
        assert!(phase2_resolve_toolbar_submit_binding());
    }

    #[test]
    fn phase2_input_registry_resolves_toolbar_nav_reload_binding() {
        assert!(phase2_resolve_input_binding(
            crate::shell::desktop::runtime::registries::input::INPUT_BINDING_TOOLBAR_NAV_RELOAD,
        ));
    }

    #[test]
    fn phase2_lens_registry_resolves_default_lens_id() {
        let lens =
            phase2_resolve_lens(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT);
        assert_eq!(lens.name, "Default");
        assert_eq!(
            lens.lens_id.as_deref(),
            Some(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT)
        );
        assert_eq!(lens.physics.name, "Liquid");
        assert!(matches!(
            lens.layout,
            crate::registries::atomic::lens::LayoutMode::Free
        ));
        assert_eq!(
            lens.theme.as_ref().map(|theme| theme.background_rgb),
            Some((20, 20, 25))
        );
    }

    #[test]
    fn phase2_lens_registry_falls_back_for_unknown_lens_id() {
        let lens = phase2_resolve_lens("lens:unknown");
        assert_eq!(lens.name, "Default");
        assert_eq!(
            lens.lens_id.as_deref(),
            Some(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT)
        );
    }

    #[test]
    fn phase2_lens_resolution_preserves_direct_values() {
        let mut lens = crate::app::LensConfig::default();
        lens.physics = crate::registries::atomic::lens::PhysicsProfile::gas();
        lens.layout = crate::registries::atomic::lens::LayoutMode::Grid { gap: 32.0 };
        lens.theme = Some(crate::registries::atomic::lens::ThemeData {
            background_rgb: (1, 2, 3),
            accent_rgb: (4, 5, 6),
            font_scale: 1.2,
            stroke_width: 2.0,
        });

        assert_eq!(lens.physics.name, "Gas");
        assert!(matches!(
            lens.layout,
            crate::registries::atomic::lens::LayoutMode::Grid { gap: 32.0 }
        ));
        assert_eq!(
            lens.theme.as_ref().map(|theme| theme.background_rgb),
            Some((1, 2, 3))
        );
    }

    #[test]
    fn phase2_action_registry_omnibox_search_selects_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://example.com".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example Handle".into();
        }

        let intents = phase2_execute_omnibox_node_search_action(&app, "example handle");
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SelectNode { key: selected, .. }) if *selected == key
        ));
    }

    #[test]
    fn phase2_action_registry_graph_submit_updates_selected_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://start.com".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.select_node(key, false);

        let result = phase2_execute_graph_view_submit_action(&app, "https://next.com");
        assert!(result.open_selected_tile);
        assert!(matches!(
            result.mutations.first(),
            Some(GraphMutation::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://next.com"
        ));
    }

    #[test]
    fn phase2_action_registry_detail_submit_updates_focused_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://start.com".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let result =
            phase2_execute_detail_view_submit_action(&app, "https://detail-next.com", Some(key));

        assert!(!result.open_selected_tile);
        assert!(matches!(
            result.mutations.first(),
            Some(GraphMutation::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://detail-next.com"
        ));
        assert!(matches!(
            result.runtime_events.first(),
            Some(RuntimeEvent::PromoteNodeToActive { key: selected, .. }) if *selected == key
        ));
    }

    #[test]
    fn phase5_action_registry_sync_now_emits_sync_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let intents = phase5_execute_verse_sync_now_action(&app);

        assert_eq!(intents.len(), 1);
        assert!(matches!(intents.first(), Some(GraphIntent::SyncNow)));
    }

    #[test]
    fn phase5_action_registry_forget_device_emits_peer_targeted_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng())
            .public()
            .to_string();

        let intents = phase5_execute_verse_forget_device_action(&app, &peer_id);

        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::ForgetDevice { peer_id: emitted }) if emitted == &peer_id
        ));
    }

    #[test]
    fn phase5_action_registry_pair_local_peer_emits_trust_peer_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng())
            .public()
            .to_string();

        let intents = phase5_execute_verse_pair_local_peer_action(&app, &peer_id);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::TrustPeer { peer_id: emitted, .. }) if emitted == &peer_id
        ));
    }

    #[test]
    fn phase5_action_registry_share_workspace_emits_grant_access_intents() {
        let app = GraphBrowserApp::new_for_testing();
        let intents = phase5_execute_verse_share_workspace_action(&app, "workspace:test");
        assert!(intents.is_empty() || intents.iter().all(|intent| {
            matches!(
                intent,
                GraphIntent::GrantWorkspaceAccess { workspace_id, .. }
                    if workspace_id == "workspace:test"
            )
        }));
    }

    #[test]
    fn phase0_normalization_rewrites_unknown_scheme_to_protocol_fallback() {
        let parsed = ServoUrl::parse("foo://example.com/path").expect("url should parse");
        let rewritten =
            phase0_decide_navigation_with_control(parsed, None, ProtocolResolveControl::default())
                .expect("default protocol resolve control should not cancel")
                .normalized_url;

        assert_eq!(rewritten.scheme(), "https");
        assert_eq!(rewritten.host_str(), Some("example.com"));
    }

    #[test]
    fn phase0_decision_prefers_mime_hint_for_viewer_selection() {
        let parsed = ServoUrl::parse("https://example.com/file.bin").expect("url should parse");
        let decision = phase0_decide_navigation_with_control(
            parsed,
            Some("text/csv"),
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel");

        assert_eq!(decision.viewer.viewer_id, "viewer:csv");
        assert_eq!(decision.viewer.matched_by, "mime");
        assert_eq!(decision.normalized_url.scheme(), "https");
    }

    #[test]
    fn phase0_decision_uses_protocol_inferred_mime_hint_when_explicit_hint_missing() {
        let parsed =
            ServoUrl::parse("https://example.com/download/no_extension").expect("url should parse");
        let decision =
            phase0_decide_navigation_with_control(parsed, None, ProtocolResolveControl::default())
                .expect("default protocol resolve control should not cancel");

        assert_eq!(decision.protocol.inferred_mime_hint.as_deref(), None);
        assert_eq!(decision.viewer.viewer_id, "viewer:webview");

        let data_uri = ServoUrl::parse("data:text/csv,foo,bar").expect("data URI should parse");
        let data_decision = phase0_decide_navigation_with_control(
            data_uri,
            None,
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel");

        assert_eq!(
            data_decision.protocol.inferred_mime_hint.as_deref(),
            Some("text/csv")
        );
        assert_eq!(data_decision.viewer.viewer_id, "viewer:csv");
        assert_eq!(data_decision.viewer.matched_by, "mime");
    }

    #[test]
    fn phase0_decision_prefers_explicit_mime_hint_over_protocol_inferred_hint() {
        let parsed = ServoUrl::parse("data:text/csv,foo,bar").expect("data URI should parse");
        let decision = phase0_decide_navigation_with_control(
            parsed,
            Some("application/pdf"),
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel");

        assert_eq!(
            decision.protocol.inferred_mime_hint.as_deref(),
            Some("text/csv")
        );
        assert_eq!(decision.viewer.matched_by, "mime");
        assert_ne!(decision.viewer.viewer_id, "viewer:csv");
    }

    #[test]
    fn phase0_decision_returns_none_when_protocol_resolution_is_cancelled() {
        let parsed = ServoUrl::parse("https://example.com/readme.md").expect("url should parse");

        let decision = phase0_decide_navigation_with_control(
            parsed,
            None,
            ProtocolResolveControl::cancelled(),
        );

        assert!(decision.is_none());
    }

    #[test]
    fn phase3_viewer_surface_resolution_returns_default_profile() {
        let resolution = phase3_resolve_viewer_surface_profile("viewer:webview");
        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_DEFAULT);
    }

    #[test]
    fn phase3_nostr_sign_event_returns_signed_payload() {
        let _guard = nostr_backend_test_guard();
        phase3_nostr_use_local_signer();
        let unsigned = NostrUnsignedEvent {
            kind: 1,
            content: "hello".to_string(),
            tags: vec![("t".to_string(), "graphshell".to_string())],
        };

        let signed = phase3_nostr_sign_event("default", &unsigned)
            .expect("nostr sign scaffold should return signed payload");
        assert_eq!(signed.kind, 1);
        assert_eq!(signed.signature.len(), 128);
        assert_eq!(signed.event_id.len(), 64);
    }

    #[test]
    fn phase3_nostr_subscribe_unsubscribe_scaffold_roundtrip() {
        let handle = phase3_nostr_relay_subscribe(
            Some("timeline"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec![],
            },
        )
        .expect("nostr relay subscribe scaffold should accept non-empty filter");

        assert_eq!(handle.id, "timeline");
        assert!(phase3_nostr_relay_unsubscribe(&handle));
    }

    #[test]
    fn phase3_nostr_caller_scoping_prevents_cross_caller_unsubscribe() {
        let handle = phase3_nostr_relay_subscribe_for_caller(
            "mod:alpha",
            Some("shared"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.damus.io".to_string()],
            },
        )
        .expect("caller-scoped subscribe should succeed");

        assert!(!phase3_nostr_relay_unsubscribe_for_caller(
            "mod:beta", &handle
        ));
        assert!(phase3_nostr_relay_unsubscribe_for_caller(
            "mod:alpha",
            &handle
        ));
    }

    #[test]
    fn phase3_nostr_publish_to_relays_for_caller_accepts_explicit_targets() {
        let publish = phase3_nostr_relay_publish_to_relays_for_caller(
            "mod:alpha",
            &NostrSignedEvent {
                event_id: "evt-typed-caller".to_string(),
                pubkey: "pk".to_string(),
                signature: "sig".to_string(),
                kind: 1,
                content: "hello".to_string(),
                tags: Vec::new(),
            },
            &["wss://relay.damus.io".to_string()],
        )
        .expect("publish-to-relays should accept explicit default relay target");

        assert!(publish.accepted);
        assert_eq!(publish.relay_count, 1);
    }

    #[test]
    fn phase3_nostr_publish_scaffold_rejects_empty_signature() {
        let _guard = nostr_backend_test_guard();
        phase3_nostr_use_local_signer();
        let publish = phase3_nostr_relay_publish(&NostrSignedEvent {
            event_id: "evt".to_string(),
            pubkey: "pk".to_string(),
            signature: String::new(),
            kind: 1,
            content: "bad".to_string(),
            tags: Vec::new(),
        });

        assert!(publish.is_err());
    }

    #[test]
    fn phase3_nostr_nip46_backend_reports_unavailable() {
        let _guard = nostr_backend_test_guard();
        phase3_nostr_use_nip46_signer("wss://relay.example", "npub1delegate")
            .expect("nip46 config should be accepted");

        let result = phase3_nostr_sign_event(
            "default",
            &NostrUnsignedEvent {
                kind: 1,
                content: "hello".to_string(),
                tags: Vec::new(),
            },
        );
        assert!(matches!(result, Err(NostrCoreError::BackendUnavailable(_))));

        // Restore default local backend to avoid leaking state into other tests.
        phase3_nostr_use_local_signer();
    }

    #[test]
    fn diagnostics_registry_accepts_namespaced_mod_channel_registration() {
        let result = register_mod_diagnostics_channel(
            "planner",
            "mod.planner.agent.think",
            1,
            Some("planner thought loop".to_string()),
        )
        .expect("mod channel registration should succeed");

        assert!(result);
    }

    #[test]
    fn diagnostics_registry_rejects_non_namespaced_verse_channel_registration() {
        let result = register_verse_diagnostics_channel(
            "peer-a",
            "agent.decide",
            1,
            Some("invalid namespace".to_string()),
        );

        assert!(matches!(
            result,
            Err(diagnostics::ChannelRegistrationError::InvalidOwnership { .. })
        ));
    }

    #[test]
    fn diagnostics_registry_registers_invariant_extension() {
        let created = register_diagnostics_invariant(diagnostics::DiagnosticsInvariant {
            invariant_id: "invariant.registry.test.layout_timeout".to_string(),
            start_channel: "layout.compute_started".to_string(),
            terminal_channels: vec![
                "layout.compute_succeeded".to_string(),
                "layout.compute_failed".to_string(),
            ],
            timeout_ms: 250,
            owner: diagnostics::DiagnosticsChannelOwner::core(),
            enabled: true,
        })
        .expect("invariant registration should succeed");

        assert!(created);
    }
}
