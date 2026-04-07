pub(crate) mod action;
pub(crate) mod agent;
pub(crate) mod canvas;
pub(crate) mod identity;
pub(crate) mod index;
pub(crate) mod input;
pub(crate) mod knowledge;
pub(crate) mod layout;
pub(crate) mod lens;
pub(crate) mod nostr_core;
pub(crate) mod physics_profile;
pub(crate) mod protocol;
pub(crate) mod renderer;
pub(crate) mod signal_routing;
pub(crate) mod theme;
pub(crate) mod workbench_surface;
pub(crate) mod workflow;

use std::sync::{Arc, Mutex, OnceLock};

use sysinfo::System;

use crate::mods::native::verso::finger::{FingerRegistry, FingerServerHandle};
use crate::mods::native::verso::gemini::{CapsuleRegistry, GeminiServerHandle};
use crate::mods::native::verso::gopher::{GopherRegistry, GopherServerHandle};

static GEMINI_REGISTRY: OnceLock<CapsuleRegistry> = OnceLock::new();
static GEMINI_SERVER_HANDLE: Mutex<Option<GeminiServerHandle>> = Mutex::new(None);
static GOPHER_REGISTRY: OnceLock<GopherRegistry> = OnceLock::new();
static GOPHER_SERVER_HANDLE: Mutex<Option<GopherServerHandle>> = Mutex::new(None);
static FINGER_REGISTRY: OnceLock<FingerRegistry> = OnceLock::new();
static FINGER_SERVER_HANDLE: Mutex<Option<FingerServerHandle>> = Mutex::new(None);

fn gemini_registry() -> &'static CapsuleRegistry {
    GEMINI_REGISTRY.get_or_init(CapsuleRegistry::new)
}
fn gopher_registry() -> &'static GopherRegistry {
    GOPHER_REGISTRY.get_or_init(GopherRegistry::new)
}
fn finger_registry() -> &'static FingerRegistry {
    FINGER_REGISTRY.get_or_init(FingerRegistry::new)
}

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphMutation, MemoryPressureLevel, RendererId, RuntimeEvent,
    WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::registries::atomic::ProtocolHandlerProviders;
use crate::registries::atomic::ViewerHandlerProviders;
use crate::registries::atomic::diagnostics;
use crate::registries::atomic::lens::LensRegistry;
use crate::registries::atomic::protocol::ProtocolContractRegistry;
use crate::registries::atomic::viewer::{ViewerCapability, ViewerRegistry, ViewerSelection};
use crate::registries::domain::layout::ConformanceLevel;
use crate::registries::domain::layout::LayoutDomainRegistry;
use crate::registries::domain::layout::canvas::{CanvasLassoBinding, CanvasSurfaceResolution};
use crate::registries::domain::layout::viewer_surface::ViewerSurfaceResolution;
use crate::registries::domain::presentation::{
    PresentationDomainProfileResolution, PresentationDomainRegistry,
};
use crate::registries::infrastructure::{ModExtensionRecord, ModRegistry, ModUnloadError};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::workbench::pane_model::PaneId;
use action::{
    ACTION_DETAIL_VIEW_SUBMIT, ACTION_GRAPH_VIEW_SUBMIT, ACTION_OMNIBOX_NODE_SEARCH,
    ACTION_VERSE_FORGET_DEVICE, ACTION_VERSE_PAIR_DEVICE, ACTION_VERSE_SHARE_WORKSPACE,
    ACTION_VERSE_SYNC_NOW, ActionCapability, ActionDispatch, ActionFailure, ActionOutcome,
    ActionPayload, ActionRegistry, PairingMode, RuntimeAction,
};
use agent::{Agent, AgentDescriptor, AgentRegistry};
use canvas::CanvasRegistry;
use diagnostics::DiagnosticsRegistry;
use identity::{IdentityRegistry, PresenceBindingAssertion};
use index::{IndexRegistry, SearchResult};
use input::{
    InputActionBindingDescriptor, InputBinding, InputBindingRemap,
    InputConflict as InputRemapConflict, InputContext, InputRegistry,
};
use knowledge::{KnowledgeRegistry, SemanticReconcileReport, TagValidationResult};
use layout::LayoutRegistry;
pub(crate) use nostr_core::{
    Nip07PermissionDecision, Nip07PermissionGrant, NostrSignerBackendSnapshot,
    PersistedNostrSignerSettings, PersistedNostrSubscription,
};
use nostr_core::{
    Nip46PermissionDecision, NostrCoreError, NostrCoreRegistry, NostrFilterSet,
    NostrPublishReceipt, NostrSignedEvent, NostrSubscriptionHandle, NostrUnsignedEvent,
    ParsedNip46BunkerUri,
};
use physics_profile::PhysicsProfileRegistry;
use protocol::{
    ProtocolRegistry, ProtocolResolution, ProtocolResolveControl, ProtocolResolveOutcome,
};
use renderer::{PaneAttachment, RendererRegistry, RendererRegistryError};
use servo::ServoUrl;
use signal_routing::{
    AsyncSignalSubscription, InputEventSignal, LifecycleSignal, NavigationSignal, ObserverId,
    RegistryEventSignal, SignalBus, SignalEnvelope, SignalKind, SignalRoutingLayer, SignalSource,
    SignalTopic,
};
use theme::{ThemeCapability, ThemeRegistry, ThemeResolution};
use workbench_surface::{
    WorkbenchSurfaceDescription, WorkbenchSurfaceRegistry, WorkbenchSurfaceResolution,
};
use workflow::{WorkflowActivation, WorkflowActivationError, WorkflowCapability, WorkflowRegistry};

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
pub(crate) const CHANNEL_IDENTITY_VERIFY_STARTED: &str = "registry.identity.verify_started";
pub(crate) const CHANNEL_IDENTITY_VERIFY_SUCCEEDED: &str = "registry.identity.verify_succeeded";
pub(crate) const CHANNEL_IDENTITY_VERIFY_FAILED: &str = "registry.identity.verify_failed";
pub(crate) const CHANNEL_IDENTITY_KEY_UNAVAILABLE: &str = "registry.identity.key_unavailable";
pub(crate) const CHANNEL_IDENTITY_KEY_LOADED: &str = "registry.identity.key_loaded";
pub(crate) const CHANNEL_IDENTITY_KEY_GENERATED: &str = "registry.identity.key_generated";
pub(crate) const CHANNEL_IDENTITY_TRUST_STORE_LOAD_FAILED: &str =
    "registry.identity.trust_store_load_failed";
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
pub(crate) const CHANNEL_UI_GRAPH_FIT_GRAPHLET_FALLBACK_TO_FIT: &str =
    "runtime.ui.graph.fit_graphlet_fallback_to_fit";
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
pub(crate) const CHANNEL_UI_GRAPH_VIEW_REGION_MUTATION_APPLIED: &str =
    "runtime.ui.graph.view_region_mutation_applied";
pub(crate) const CHANNEL_UI_GRAPH_VIEW_TRANSFER_SUCCEEDED: &str =
    "runtime.ui.graph.view_transfer_succeeded";
pub(crate) const CHANNEL_UI_GRAPH_VIEW_TRANSFER_BLOCKED: &str =
    "runtime.ui.graph.view_transfer_blocked";
pub(crate) const CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED: &str =
    "runtime.ui.command_bar.command_palette_requested";
pub(crate) const CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED: &str =
    "runtime.ui.command_bar.workbench_command.requested";
pub(crate) const CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED: &str =
    "runtime.ui.command_bar.workbench_command.executed";
pub(crate) const CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS: &str =
    "runtime.ui.command_bar.workbench_command.blocked_by_focus";
pub(crate) const CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED: &str =
    "runtime.ui.command_surface.route_resolved";
pub(crate) const CHANNEL_UI_COMMAND_SURFACE_ROUTE_BLOCKED: &str =
    "runtime.ui.command_surface.route_blocked";
pub(crate) const CHANNEL_UI_COMMAND_SURFACE_ROUTE_FALLBACK: &str =
    "runtime.ui.command_surface.route_fallback";
pub(crate) const CHANNEL_UI_COMMAND_SURFACE_ROUTE_NO_TARGET: &str =
    "runtime.ui.command_surface.route_no_target";
pub(crate) const CHANNEL_UI_COMMAND_BAR_NAV_ACTION_REQUESTED: &str =
    "runtime.ui.command_bar.nav_action.requested";
pub(crate) const CHANNEL_UI_COMMAND_BAR_NAV_ACTION_BLOCKED: &str =
    "runtime.ui.command_bar.nav_action.blocked";
pub(crate) const CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET: &str =
    "runtime.ui.command_bar.nav_action.no_target";
pub(crate) const CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_REQUESTED: &str =
    "runtime.host.webdriver.browser_action.requested";
pub(crate) const CHANNEL_HOST_WEBDRIVER_BROWSER_ACTION_MISSING_WEBVIEW: &str =
    "runtime.host.webdriver.browser_action.missing_webview";
pub(crate) const CHANNEL_HOST_WEBDRIVER_LOAD_URL_REQUESTED: &str =
    "runtime.host.webdriver.load_url.requested";
pub(crate) const CHANNEL_HOST_WEBDRIVER_LOAD_URL_MISSING_WEBVIEW: &str =
    "runtime.host.webdriver.load_url.missing_webview";
pub(crate) const CHANNEL_HOST_WEBDRIVER_LOAD_STATUS_BLOCKED: &str =
    "runtime.host.webdriver.load_status.blocked";
pub(crate) const CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_REQUEST_STARTED: &str =
    "runtime.ui.omnibar.provider_mailbox.request_started";
pub(crate) const CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_APPLIED: &str =
    "runtime.ui.omnibar.provider_mailbox.applied";
pub(crate) const CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_FAILED: &str =
    "runtime.ui.omnibar.provider_mailbox.failed";
pub(crate) const CHANNEL_UI_OMNIBAR_PROVIDER_MAILBOX_STALE: &str =
    "runtime.ui.omnibar.provider_mailbox.stale";
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
pub(crate) const CHANNEL_NOSTR_RELAY_CONNECT_STARTED: &str = "mod.nostrcore.relay_connect_started";
pub(crate) const CHANNEL_NOSTR_RELAY_CONNECT_SUCCEEDED: &str =
    "mod.nostrcore.relay_connect_succeeded";
pub(crate) const CHANNEL_NOSTR_RELAY_CONNECT_FAILED: &str = "mod.nostrcore.relay_connect_failed";
pub(crate) const CHANNEL_NOSTR_RELAY_DISCONNECTED: &str = "mod.nostrcore.relay_disconnected";
pub(crate) const CHANNEL_NOSTR_INTENT_REJECTED: &str = "mod.nostrcore.intent_rejected";
pub(crate) const CHANNEL_NOSTR_SECURITY_VIOLATION: &str = "mod.nostrcore.security_violation";
pub(crate) const CHANNEL_COMPOSITOR_GL_STATE_VIOLATION: &str = "compositor.gl_state_violation";
pub(crate) const CHANNEL_COMPOSITOR_CONTENT_PASS_REGISTERED: &str =
    "compositor.content_pass_registered";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_PASS_REGISTERED: &str =
    "compositor.overlay_pass_registered";
pub(crate) const CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION: &str = "compositor.pass_order_violation";
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
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_TILE_DRAG: &str =
    "compositor.overlay.native.suppressed.tile_drag";
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
pub(crate) const CHANNEL_COMPOSITOR_TILE_ACTIVITY: &str = "compositor:tile_activity";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_LIFECYCLE_INDICATOR: &str =
    "compositor:overlay_lifecycle_indicator";
pub(crate) const CHANNEL_COMPOSITOR_LENS_OVERLAY_APPLIED: &str = "compositor:lens_overlay_applied";
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
pub(crate) const CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH: &str =
    "ux:arrangement_projection_health";
pub(crate) const CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK: &str =
    "ux:arrangement_missing_family_fallback";
pub(crate) const CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION: &str =
    "ux:arrangement_durability_transition";
pub(crate) const CHANNEL_UX_FOCUS_CAPTURE_ENTER: &str = "ux:focus_capture_enter";
pub(crate) const CHANNEL_UX_FOCUS_CAPTURE_EXIT: &str = "ux:focus_capture_exit";
pub(crate) const CHANNEL_UX_FOCUS_RETURN_FALLBACK: &str = "ux:focus_return_fallback";
pub(crate) const CHANNEL_UX_FOCUS_REALIZATION_MISMATCH: &str = "ux:focus_realization_mismatch";
pub(crate) const CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM: &str = "ux:embedded_focus_reclaim";
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
pub(crate) const CHANNEL_UX_PRESENTATION_BOUNDS_MISSING: &str = "ux:presentation_bounds_missing";
pub(crate) const CHANNEL_UX_LAYOUT_GUTTER_DETECTED: &str = "ux:layout_gutter_detected";
pub(crate) const CHANNEL_UX_LAYOUT_OVERLAP_DETECTED: &str = "ux:layout_overlap_detected";
pub(crate) const CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT: &str = "ux:layout_constraint_conflict";
pub(crate) const CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT: &str = "ux:layout_constraint_drift";
pub(crate) const CHANNEL_UX_CONFIG_MODE_ENTERED: &str = "ux:config_mode_entered";
pub(crate) const CHANNEL_UX_FIRST_USE_PROMPT_SHOWN: &str = "ux:first_use_prompt_shown";
pub(crate) const CHANNEL_COMPOSITOR_PAINT_NOT_CONFIRMED: &str = "compositor.paint_not_confirmed";
pub(crate) const CHANNEL_COMPOSITOR_NATIVE_OVERLAY_RECT_MISMATCH: &str =
    "compositor.native_overlay_rect_mismatch";
pub(crate) const CHANNEL_UX_PROBE_REGISTERED: &str = "ux:probe_registered";
pub(crate) const CHANNEL_UX_PROBE_DISABLED: &str = "ux:probe_disabled";
pub(crate) const CHANNEL_UX_FACET_FILTER_APPLIED: &str = "ux:facet_filter_applied";
pub(crate) const CHANNEL_UX_FACET_FILTER_CLEARED: &str = "ux:facet_filter_cleared";
pub(crate) const CHANNEL_UX_FACET_FILTER_INVALID_QUERY: &str = "ux:facet_filter_invalid_query";
pub(crate) const CHANNEL_UX_FACET_FILTER_TYPE_MISMATCH: &str = "ux:facet_filter_type_mismatch";
pub(crate) const CHANNEL_UX_FACET_FILTER_EVAL_FAILURE: &str = "ux:facet_filter_eval_failure";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_PUBLISHED: &str =
    "register.signal_routing.published";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_UNROUTED: &str =
    "register.signal_routing.unrouted";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED: &str = "register.signal_routing.failed";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH: &str =
    "register.signal_routing.queue_depth";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_LAGGED: &str = "register.signal_routing.lagged";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_MOD_WORKFLOW_ROUTED: &str =
    "register.signal_routing.mod_workflow_routed";
pub(crate) const CHANNEL_REGISTER_SIGNAL_ROUTING_SUBSYSTEM_HEALTH_PROPAGATED: &str =
    "register.signal_routing.subsystem_health_propagated";
pub(crate) const CHANNEL_WORKBENCH_SURFACE_PROFILE_ACTIVATED: &str =
    "registry.workbench_surface.profile_activated";
pub(crate) const CHANNEL_CANVAS_PROFILE_ACTIVATED: &str = "registry.canvas.profile_activated";
pub(crate) const CHANNEL_CANVAS_FRAME_AFFINITY_CHANGED: &str =
    "registry.canvas.frame_affinity_changed";
pub(crate) const CHANNEL_PHYSICS_PROFILE_ACTIVATED: &str = "registry.physics_profile.activated";
pub(crate) const CHANNEL_LAYOUT_COMPUTE_STARTED: &str = "registry.layout.compute_started";
pub(crate) const CHANNEL_LAYOUT_COMPUTE_SUCCEEDED: &str = "registry.layout.compute_succeeded";
pub(crate) const CHANNEL_LAYOUT_COMPUTE_FAILED: &str = "registry.layout.compute_failed";
pub(crate) const CHANNEL_LAYOUT_FALLBACK_USED: &str = "registry.layout.fallback_used";
pub(crate) const CHANNEL_LAYOUT_DOMAIN_PROFILE_RESOLVED: &str =
    "registry.layout_domain.profile_resolved";
pub(crate) const CHANNEL_PRESENTATION_PROFILE_RESOLVED: &str =
    "registry.presentation.profile_resolved";
pub(crate) const CHANNEL_THEME_ACTIVATED: &str = "registry.theme.activated";
pub(crate) const CHANNEL_AGENT_SPAWNED: &str = "registry.agent.spawned";
pub(crate) const CHANNEL_AGENT_INTENT_DROPPED: &str = "registry.agent.intent_dropped";
pub(crate) const CHANNEL_WORKFLOW_ACTIVATED: &str = "registry.workflow.activated";
pub(crate) const CHANNEL_KNOWLEDGE_INDEX_UPDATED: &str = "registry.knowledge.index_updated";
pub(crate) const CHANNEL_KNOWLEDGE_TAG_VALIDATION_WARN: &str =
    "registry.knowledge.tag_validation_warn";
pub(crate) const CHANNEL_KNOWLEDGE_PLACEMENT_ANCHOR_SELECTED: &str =
    "registry.knowledge.placement_anchor_selected";
pub(crate) const CHANNEL_KNOWLEDGE_CLASSIFICATION_CLUSTERING_APPLIED: &str =
    "registry.knowledge.classification_clustering_applied";
pub(crate) const CHANNEL_INDEX_SEARCH: &str = "registry.index.search";

pub(crate) const CHANNEL_SYSTEM_TASK_BUDGET_BACKPRESSURE: &str = "system:task_budget:backpressure";
pub(crate) const CHANNEL_SYSTEM_TASK_BUDGET_WORKER_SUSPENDED: &str =
    "system:task_budget:worker_suspended";
pub(crate) const CHANNEL_SYSTEM_TASK_BUDGET_WORKER_RESUMED: &str =
    "system:task_budget:worker_resumed";
pub(crate) const CHANNEL_SYSTEM_TASK_BUDGET_QUEUE_DEPTH: &str = "system:task_budget:queue_depth";

static REGISTRY_RUNTIME: OnceLock<Arc<RegistryRuntime>> = OnceLock::new();

fn runtime() -> &'static RegistryRuntime {
    REGISTRY_RUNTIME
        .get_or_init(|| Arc::new(RegistryRuntime::new_with_mods()))
        .as_ref()
}

pub(crate) fn shared_runtime() -> Arc<RegistryRuntime> {
    REGISTRY_RUNTIME
        .get_or_init(|| Arc::new(RegistryRuntime::new_with_mods()))
        .clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Phase0NavigationDecision {
    pub(crate) normalized_url: ServoUrl,
    pub(crate) protocol: ProtocolResolution,
    pub(crate) viewer: ViewerSelection,
}

pub(crate) struct RegistryRuntime {
    #[allow(dead_code)]
    pub(crate) diagnostics: DiagnosticsRegistry,
    #[allow(dead_code)]
    signal_bus: Arc<dyn SignalBus>,
    #[allow(dead_code)]
    identity: Mutex<IdentityRegistry>,
    dynamic: Mutex<DynamicRegistrySurfaces>,
    input: Mutex<InputRegistry>,
    layout: Mutex<LayoutRegistry>,
    #[allow(dead_code)]
    nostr_core: NostrCoreRegistry,
    agent: Mutex<AgentRegistry>,
    canvas: Mutex<CanvasRegistry>,
    layout_domain: LayoutDomainRegistry,
    presentation: PresentationDomainRegistry,
    physics_profile: Mutex<PhysicsProfileRegistry>,
    #[allow(dead_code)]
    renderer: Mutex<RendererRegistry>,
    workflow: Mutex<WorkflowRegistry>,
    workbench_surface: Mutex<WorkbenchSurfaceRegistry>,
    pub(crate) knowledge: KnowledgeRegistry,
    mod_registry: Mutex<ModRegistry>,
    /// When `true` (default), `WindowEvent::ThemeChanged` drives the active theme.
    /// Set to `false` when the user pins an explicit Light or Dark mode preference.
    theme_follows_system: std::sync::atomic::AtomicBool,
}

struct DynamicRegistrySurfaces {
    action: ActionRegistry,
    lens: LensRegistry,
    protocol: ProtocolRegistry,
    theme: ThemeRegistry,
    viewer: ViewerRegistry,
    index: IndexRegistry,
}

impl DynamicRegistrySurfaces {
    fn apply_extension(&mut self, record: &ModExtensionRecord) -> Result<(), String> {
        match record {
            ModExtensionRecord::ProtocolScheme { scheme, .. } => {
                self.protocol.register_scheme(scheme);
            }
            ModExtensionRecord::ViewerMime {
                mime,
                previous_viewer_id: _,
            } => {
                let viewer_id = static_viewer_id_for_runtime_mime(mime)
                    .ok_or_else(|| format!("unsupported viewer mapping for mime {mime}"))?;
                self.viewer.register_mime(mime, viewer_id);
            }
            ModExtensionRecord::ViewerExtension {
                extension,
                previous_viewer_id: _,
            } => {
                let viewer_id =
                    static_viewer_id_for_runtime_extension(extension).ok_or_else(|| {
                        format!("unsupported viewer mapping for extension {extension}")
                    })?;
                self.viewer.register_extension(extension, viewer_id);
            }
            ModExtensionRecord::ViewerCapabilities {
                viewer_id,
                previous_capabilities: _,
            } => {
                let static_viewer_id = static_viewer_id(viewer_id)
                    .ok_or_else(|| format!("unsupported viewer capabilities for {viewer_id}"))?;
                self.viewer.register_capabilities(
                    static_viewer_id,
                    self.viewer.capabilities_for(static_viewer_id),
                );
            }
            ModExtensionRecord::Action { .. }
            | ModExtensionRecord::IndexProvider { .. }
            | ModExtensionRecord::Lens { .. }
            | ModExtensionRecord::Theme { .. } => {}
        }
        Ok(())
    }

    fn remove_extension(&mut self, record: ModExtensionRecord) -> Result<(), String> {
        match record {
            ModExtensionRecord::ProtocolScheme {
                scheme,
                previously_present,
            } => {
                if !previously_present {
                    self.protocol.unregister_scheme(&scheme);
                }
            }
            ModExtensionRecord::ViewerMime {
                mime,
                previous_viewer_id,
            } => match previous_viewer_id.as_deref().and_then(static_viewer_id) {
                Some(previous) => {
                    self.viewer.register_mime(&mime, previous);
                }
                None => {
                    self.viewer.unregister_mime(&mime);
                }
            },
            ModExtensionRecord::ViewerExtension {
                extension,
                previous_viewer_id,
            } => match previous_viewer_id.as_deref().and_then(static_viewer_id) {
                Some(previous) => {
                    self.viewer.register_extension(&extension, previous);
                }
                None => {
                    self.viewer.unregister_extension(&extension);
                }
            },
            ModExtensionRecord::ViewerCapabilities {
                viewer_id,
                previous_capabilities,
            } => {
                let static_viewer_id = static_viewer_id(&viewer_id).ok_or_else(|| {
                    format!("unsupported viewer capability rollback for {viewer_id}")
                })?;
                if let Some(previous) = previous_capabilities {
                    self.viewer
                        .register_capabilities(static_viewer_id, previous);
                } else {
                    self.viewer.unregister_capabilities(static_viewer_id);
                }
            }
            ModExtensionRecord::Action { action_id } => {
                self.action.unregister(&action_id);
            }
            ModExtensionRecord::IndexProvider { provider_id } => {
                self.index.unregister_provider(&provider_id);
            }
            ModExtensionRecord::Lens { lens_id } => {
                self.lens.unregister(&lens_id);
            }
            ModExtensionRecord::Theme { theme_id } => {
                self.theme.unregister_theme(&theme_id);
            }
        }
        Ok(())
    }
}

fn static_viewer_id(viewer_id: &str) -> Option<&'static str> {
    match viewer_id {
        "viewer:webview" => Some("viewer:webview"),
        "viewer:wry" => Some("viewer:wry"),
        "viewer:plaintext" => Some("viewer:plaintext"),
        "viewer:markdown" => Some("viewer:markdown"),
        "viewer:image" => Some("viewer:image"),
        "viewer:directory" => Some("viewer:directory"),
        "viewer:pdf" => Some("viewer:pdf"),
        "viewer:csv" => Some("viewer:csv"),
        "viewer:settings" => Some("viewer:settings"),
        "viewer:metadata" => Some("viewer:metadata"),
        "viewer:fallback" => Some("viewer:fallback"),
        _ => None,
    }
}

fn static_viewer_id_for_runtime_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "text/html"
        | "application/pdf"
        | "image/svg+xml"
        | "text/css"
        | "application/javascript" => Some("viewer:webview"),
        "application/x-graphshell-wry" => Some("viewer:wry"),
        _ => None,
    }
}

fn static_viewer_id_for_runtime_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "html" | "htm" | "pdf" | "svg" => Some("viewer:webview"),
        _ => None,
    }
}

fn register_verso_mod_extensions(dynamic: &mut DynamicRegistrySurfaces) -> Vec<ModExtensionRecord> {
    let mut records = Vec::new();

    for scheme in ["http", "https", "data"] {
        records.push(ModExtensionRecord::ProtocolScheme {
            scheme: scheme.to_string(),
            previously_present: dynamic.protocol.has_scheme(scheme),
        });
        dynamic.protocol.register_scheme(scheme);
    }

    for (mime, viewer_id) in [
        ("text/html", "viewer:webview"),
        ("image/svg+xml", "viewer:webview"),
        ("text/css", "viewer:webview"),
        ("application/javascript", "viewer:webview"),
    ] {
        let previous = dynamic.viewer.register_mime(mime, viewer_id);
        records.push(ModExtensionRecord::ViewerMime {
            mime: mime.to_string(),
            previous_viewer_id: previous.map(str::to_string),
        });
    }
    // When the native PDF viewer feature is compiled in, let its registration
    // from ViewerRegistry::default() stand.  Otherwise Servo handles PDFs.
    #[cfg(not(feature = "pdf"))]
    {
        let previous = dynamic
            .viewer
            .register_mime("application/pdf", "viewer:webview");
        records.push(ModExtensionRecord::ViewerMime {
            mime: "application/pdf".to_string(),
            previous_viewer_id: previous.map(str::to_string),
        });
    }

    for (extension, viewer_id) in [
        ("html", "viewer:webview"),
        ("htm", "viewer:webview"),
        ("svg", "viewer:webview"),
    ] {
        let previous = dynamic.viewer.register_extension(extension, viewer_id);
        records.push(ModExtensionRecord::ViewerExtension {
            extension: extension.to_string(),
            previous_viewer_id: previous.map(str::to_string),
        });
    }
    #[cfg(not(feature = "pdf"))]
    {
        let previous = dynamic.viewer.register_extension("pdf", "viewer:webview");
        records.push(ModExtensionRecord::ViewerExtension {
            extension: "pdf".to_string(),
            previous_viewer_id: previous.map(str::to_string),
        });
    }

    #[cfg(feature = "wry")]
    {
        let previous = dynamic
            .viewer
            .register_mime("application/x-graphshell-wry", "viewer:wry");
        records.push(ModExtensionRecord::ViewerMime {
            mime: "application/x-graphshell-wry".to_string(),
            previous_viewer_id: previous.map(str::to_string),
        });

        // Register viewer:wry capabilities so describe_viewer("viewer:wry") returns
        // ViewerRenderMode::NativeOverlay, which is required for refresh_node_pane_render_modes
        // to set TileRenderMode::NativeOverlay and for lifecycle_reconcile to create the
        // wry overlay window.
        let previous_capabilities = dynamic.viewer.register_capabilities(
            "viewer:wry",
            crate::registries::atomic::viewer::ViewerSubsystemCapabilities {
                accessibility: crate::registries::domain::layout::CapabilityDeclaration::none(
                    "Wry accessibility bridge not yet implemented",
                ),
                security: crate::registries::domain::layout::CapabilityDeclaration::full(),
                storage: crate::registries::domain::layout::CapabilityDeclaration::none(
                    "Wry storage isolation not yet implemented",
                ),
                history: crate::registries::domain::layout::CapabilityDeclaration::none(
                    "Wry history integration not yet implemented",
                ),
            },
        );
        records.push(ModExtensionRecord::ViewerCapabilities {
            viewer_id: "viewer:wry".to_string(),
            previous_capabilities,
        });
    }

    records
}

impl Default for RegistryRuntime {
    fn default() -> Self {
        Self::new_with_registries(ProtocolRegistry::default(), ViewerRegistry::default())
    }
}

#[allow(dead_code)]
pub(crate) fn phase3_sign_identity_payload(identity_id: &str, payload: &[u8]) -> Option<String> {
    runtime().sign_identity_payload(identity_id, payload)
}

pub(crate) fn phase3_trusted_peers() -> Vec<crate::mods::native::verse::TrustedPeer> {
    runtime().trusted_peers()
}

pub(crate) fn phase3_trusted_peers_handle()
-> std::sync::Arc<std::sync::RwLock<Vec<crate::mods::native::verse::TrustedPeer>>> {
    runtime().trusted_peers_handle()
}

pub(crate) fn phase3_trust_peer(peer: crate::mods::native::verse::TrustedPeer) {
    runtime().trust_peer(peer);
}

pub(crate) fn phase3_revoke_peer(node_id: iroh::EndpointId) {
    runtime().revoke_peer(node_id);
}

pub(crate) fn phase3_grant_workspace_access(
    node_id: iroh::EndpointId,
    workspace_id: &str,
    access: crate::mods::native::verse::AccessLevel,
) {
    runtime().grant_workspace_access(node_id, workspace_id, access);
}

pub(crate) fn phase3_revoke_workspace_access(node_id: iroh::EndpointId, workspace_id: &str) {
    runtime().revoke_workspace_access(node_id, workspace_id);
}

pub(crate) fn phase3_create_presence_binding_assertion(
    audience: &str,
    ttl_secs: u64,
) -> Option<PresenceBindingAssertion> {
    runtime().create_presence_binding_assertion(audience, ttl_secs)
}

pub(crate) fn phase3_verify_presence_binding_assertion(
    assertion: &PresenceBindingAssertion,
) -> bool {
    runtime().verify_presence_binding_assertion(assertion)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_sign_event(
    persona: &str,
    unsigned: &NostrUnsignedEvent,
) -> Result<NostrSignedEvent, NostrCoreError> {
    let runtime = runtime();
    runtime.nostr_core.sign_event(
        &runtime
            .identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        persona,
        unsigned,
    )
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_use_local_signer() {
    runtime().nostr_core.use_local_signer();
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_persisted_signer_settings() -> PersistedNostrSignerSettings {
    runtime().nostr_core.persisted_signer_settings()
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_signer_backend_snapshot() -> NostrSignerBackendSnapshot {
    runtime().nostr_core.signer_backend_snapshot()
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_apply_persisted_signer_settings(
    settings: &PersistedNostrSignerSettings,
) -> Result<(), NostrCoreError> {
    runtime()
        .nostr_core
        .apply_persisted_signer_settings(settings)
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
pub(crate) fn phase3_nostr_use_nip46_bunker_uri(
    bunker_uri: &str,
) -> Result<ParsedNip46BunkerUri, NostrCoreError> {
    runtime().nostr_core.use_nip46_bunker_uri(bunker_uri)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_set_nip46_permission(
    permission: &str,
    decision: Nip46PermissionDecision,
) -> Result<(), NostrCoreError> {
    runtime()
        .nostr_core
        .set_nip46_permission(permission, decision)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_persisted_nip07_permissions() -> Vec<Nip07PermissionGrant> {
    runtime().nostr_core.persisted_nip07_permissions()
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_apply_persisted_nip07_permissions(
    permissions: &[Nip07PermissionGrant],
) -> Result<(), NostrCoreError> {
    runtime()
        .nostr_core
        .apply_persisted_nip07_permissions(permissions)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_nip07_permission_grants() -> Vec<Nip07PermissionGrant> {
    runtime().nostr_core.nip07_permission_grants()
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_set_nip07_permission(
    origin: &str,
    method: &str,
    decision: Nip07PermissionDecision,
) -> Result<(), NostrCoreError> {
    runtime()
        .nostr_core
        .set_nip07_permission(origin, method, decision)
}

#[allow(dead_code)]
pub(crate) fn phase3_nostr_nip07_request(
    origin: &str,
    method: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, NostrCoreError> {
    let runtime = runtime();
    runtime.nostr_core.nip07_request(
        &runtime
            .identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        origin,
        method,
        payload,
    )
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

pub(crate) fn phase3_nostr_persisted_subscriptions() -> Vec<PersistedNostrSubscription> {
    runtime().nostr_core.persisted_subscriptions()
}

pub(crate) fn phase3_restore_nostr_subscriptions(
    subscriptions: &[PersistedNostrSubscription],
) -> Result<usize, NostrCoreError> {
    runtime()
        .nostr_core
        .restore_persisted_subscriptions(subscriptions)
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
    fn dynamic(&self) -> std::sync::MutexGuard<'_, DynamicRegistrySurfaces> {
        self.dynamic
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn describe_action(&self, action_id: &str) -> Option<ActionCapability> {
        self.dynamic().action.describe_action(action_id)
    }

    /// Execute an action without holding the `dynamic` mutex during handler invocation.
    ///
    /// Action handlers (e.g. `omnibox:node_search`) may call back into
    /// `runtime().dynamic()` (e.g. via `phase3_index_search`). Calling
    /// `dynamic().action.execute(...)` holds the lock for the full duration of
    /// the handler, causing a deadlock on reentrant access. This method resolves
    /// the handler fn pointer while briefly holding the lock, drops it, then
    /// calls the handler lock-free.
    pub(crate) fn execute_action(
        &self,
        action_id: &str,
        app: &GraphBrowserApp,
        payload: action::ActionPayload,
    ) -> action::ActionOutcome {
        use action::{ActionFailure, ActionFailureKind, ActionOutcome};

        let resolved = self.dynamic().action.resolve(action_id);
        // Lock is dropped here before the handler runs.
        match resolved {
            None => ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::UnknownAction,
                reason: format!("unknown action: {}", action_id.to_ascii_lowercase()),
            }),
            Some((handler, capability, id)) => {
                if !action::capability_available(app, capability) {
                    return ActionOutcome::Failure(ActionFailure {
                        kind: ActionFailureKind::Rejected,
                        reason: format!(
                            "action '{}' unavailable: {}",
                            id,
                            action::capability_reason(capability)
                        ),
                    });
                }
                handler(app, &payload)
            }
        }
    }

    pub(crate) fn describe_viewer(&self, viewer_id: &str) -> Option<ViewerCapability> {
        self.dynamic().viewer.describe_viewer(viewer_id)
    }

    pub(crate) fn describe_theme(&self, theme_id: Option<&str>) -> ThemeCapability {
        self.dynamic().theme.describe_theme(theme_id)
    }

    pub(crate) fn describe_agent(&self, agent_id: &str) -> Option<AgentDescriptor> {
        self.agent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .describe(agent_id)
    }

    pub(crate) fn instantiate_agent(&self, agent_id: &str) -> Option<Box<dyn Agent>> {
        self.agent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .instantiate(agent_id)
    }

    pub(crate) fn attach_nostr_relay_worker(
        &self,
        relay_worker_tx: tokio::sync::mpsc::UnboundedSender<nostr_core::RelayWorkerCommand>,
    ) {
        self.nostr_core
            .attach_supervised_relay_worker(relay_worker_tx);
    }

    pub(crate) fn select_viewer_for_content(
        &self,
        uri: &str,
        mime_hint: Option<&str>,
    ) -> ViewerSelection {
        self.dynamic().viewer.select_for_uri(uri, mime_hint)
    }

    fn resolve_active_theme(&self, theme_id: Option<&str>) -> ThemeResolution {
        self.dynamic().theme.resolve_theme(theme_id)
    }

    fn set_active_theme(&self, theme_id: &str) -> ThemeResolution {
        let resolution = self.dynamic().theme.set_active_theme(theme_id);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_THEME_ACTIVATED,
            byte_len: resolution.resolved_id.len(),
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::ThemeChanged {
                new_theme_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn apply_system_theme_preference(&self, prefers_dark: bool) -> Option<ThemeResolution> {
        if !self
            .theme_follows_system
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        let requested = if prefers_dark {
            theme::THEME_ID_DARK
        } else {
            theme::THEME_ID_LIGHT
        };
        Some(self.set_active_theme(requested))
    }

    fn set_theme_follows_system(&self, follows: bool) {
        self.theme_follows_system
            .store(follows, std::sync::atomic::Ordering::Relaxed);
    }

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

    fn build_dynamic_surfaces(
        protocol_registry: ProtocolRegistry,
        viewer_registry: ViewerRegistry,
    ) -> DynamicRegistrySurfaces {
        DynamicRegistrySurfaces {
            action: ActionRegistry::default(),
            lens: LensRegistry::default(),
            protocol: protocol_registry,
            theme: ThemeRegistry::default(),
            viewer: viewer_registry,
            index: IndexRegistry::default(),
        }
    }

    fn new_with_registries(
        protocol_registry: ProtocolRegistry,
        viewer_registry: ViewerRegistry,
    ) -> Self {
        Self {
            diagnostics: DiagnosticsRegistry::default(),
            signal_bus: Arc::new(SignalRoutingLayer::default()),
            identity: Mutex::new(IdentityRegistry::default()),
            dynamic: Mutex::new(Self::build_dynamic_surfaces(
                protocol_registry,
                viewer_registry,
            )),
            input: Mutex::new(InputRegistry::default()),
            layout: Mutex::new(LayoutRegistry::default()),
            nostr_core: NostrCoreRegistry::default(),
            agent: Mutex::new(AgentRegistry::default()),
            canvas: Mutex::new(CanvasRegistry::default()),
            layout_domain: LayoutDomainRegistry::default(),
            presentation: PresentationDomainRegistry::default(),
            physics_profile: Mutex::new(PhysicsProfileRegistry::default()),
            renderer: Mutex::new(RendererRegistry::default()),
            workflow: Mutex::new(WorkflowRegistry::default()),
            workbench_surface: Mutex::new(WorkbenchSurfaceRegistry::default()),
            knowledge: KnowledgeRegistry::default(),
            mod_registry: Mutex::new(ModRegistry::new()),
            theme_follows_system: std::sync::atomic::AtomicBool::new(true),
        }
    }

    #[cfg(test)]
    fn new_with_provider_registries_for_tests(
        protocol_providers: ProtocolHandlerProviders,
        viewer_providers: ViewerHandlerProviders,
    ) -> Self {
        let (protocol_registry, viewer_registry) =
            Self::build_provider_wired_registries(&protocol_providers, &viewer_providers);
        Self::new_with_registries(protocol_registry, viewer_registry)
    }

    /// Create a new RegistryRuntime with mods discovered and their handlers registered.
    /// This is the standard way to initialize registries during app startup (Phase 2.4).
    ///
    /// Provider wiring for protocol/viewer paths is applied into the runtime
    /// registries returned by this constructor.
    #[allow(dead_code)]
    pub(crate) fn new_with_mods() -> Self {
        let runtime =
            Self::new_with_registries(ProtocolRegistry::default(), ViewerRegistry::default());
        {
            let mut mod_registry = runtime
                .mod_registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(e) = mod_registry.resolve_dependencies() {
                log::error!(
                    "Failed to resolve mod dependencies: {:?}. Using core seed only.",
                    e
                );
            }
        }
        if let Err(reason) = runtime.reload_dynamic_registries_from_mods() {
            log::error!(
                "Failed to load runtime mod registries: {reason}. Falling back to core seed.",
            );
        }
        runtime
    }

    fn reload_dynamic_registries_from_mods(&self) -> Result<(), String> {
        let mut mod_registry = self
            .mod_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut dynamic = self.dynamic();
        *dynamic =
            Self::build_dynamic_surfaces(ProtocolRegistry::default(), ViewerRegistry::default());
        let loaded = mod_registry.load_all_with_extensions(|mod_id| {
            Self::activate_mod_into_runtime(&mut dynamic, mod_id)
        });
        for mod_id in loaded {
            self.route_mod_lifecycle_event(&mod_id, true);
        }
        Ok(())
    }

    fn activate_mod_into_runtime(
        dynamic: &mut DynamicRegistrySurfaces,
        mod_id: &str,
    ) -> Result<Vec<ModExtensionRecord>, String> {
        match mod_id {
            "mod:verso" | "verso" => Ok(register_verso_mod_extensions(dynamic)),
            "mod:verse" | "verse" => {
                crate::mods::native::verse::activate()?;
                Ok(Vec::new())
            }
            _ => {
                let activations =
                    crate::registries::infrastructure::mod_activation::NativeModActivations::new();
                activations.activate(mod_id)?;
                Ok(Vec::new())
            }
        }
    }

    pub(crate) fn unload_mod(&self, mod_id: &str) -> Result<(), ModUnloadError> {
        let mut mod_registry = self
            .mod_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut dynamic = self.dynamic();
        let result =
            mod_registry.unload_mod_with(mod_id, |record| dynamic.remove_extension(record));
        if result.is_ok() {
            self.route_mod_lifecycle_event(mod_id, false);
        }
        result
    }

    pub(crate) fn describe_workflow(&self, workflow_id: Option<&str>) -> WorkflowCapability {
        self.workflow
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .describe_workflow(workflow_id)
    }

    pub(crate) fn describe_layout_algorithm(
        &self,
        algorithm_id: Option<&str>,
    ) -> crate::app::graph_layout::LayoutCapability {
        self.layout
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .describe_algorithm(algorithm_id)
    }

    fn resolve_layout_algorithm(
        &self,
        algorithm_id: Option<&str>,
    ) -> crate::app::graph_layout::LayoutResolution {
        self.layout
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .resolve_algorithm(algorithm_id)
    }

    fn apply_layout_algorithm_to_graph(
        &self,
        graph: &mut crate::graph::Graph,
        algorithm_id: Option<&str>,
    ) -> Result<crate::app::graph_layout::LayoutExecution, String> {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_LAYOUT_COMPUTE_STARTED,
            byte_len: algorithm_id.unwrap_or_default().len().max(1),
        });
        let mut layout = self
            .layout
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let preview = layout.resolve_algorithm(algorithm_id);
        if preview.fallback_used {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_LAYOUT_FALLBACK_USED,
                byte_len: preview.requested_id.len().max(1),
            });
        }
        match layout.apply_algorithm_to_graph(graph, algorithm_id) {
            Ok(execution) => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_LAYOUT_COMPUTE_SUCCEEDED,
                    byte_len: execution.changed_positions.max(1),
                });
                Ok(execution)
            }
            Err(error) => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_LAYOUT_COMPUTE_FAILED,
                    byte_len: error.len().max(1),
                });
                Err(error)
            }
        }
    }

    fn resolve_active_canvas_profile(&self) -> CanvasSurfaceResolution {
        self.canvas
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active_profile()
    }

    fn set_active_canvas_profile(&self, profile_id: &str) -> CanvasSurfaceResolution {
        let resolution = self
            .canvas
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_active_profile(profile_id);

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_CANVAS_PROFILE_ACTIVATED,
            byte_len: resolution.resolved_id.len(),
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::CanvasProfileChanged {
                new_profile_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn set_active_canvas_lasso_binding(
        &self,
        binding: CanvasLassoBinding,
    ) -> CanvasSurfaceResolution {
        let resolution = self
            .canvas
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_active_lasso_binding(binding);

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_CANVAS_PROFILE_ACTIVATED,
            byte_len: 1,
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::CanvasProfileChanged {
                new_profile_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn set_active_canvas_keyboard_pan_step(&self, step: f32) -> CanvasSurfaceResolution {
        let resolution = self
            .canvas
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_active_keyboard_pan_step(step);

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_CANVAS_PROFILE_ACTIVATED,
            byte_len: step.round().max(1.0) as usize,
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::CanvasProfileChanged {
                new_profile_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn set_canvas_frame_affinity_enabled(&self, enabled: bool) -> CanvasSurfaceResolution {
        let resolution = self
            .canvas
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_frame_affinity_enabled(enabled);

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_CANVAS_FRAME_AFFINITY_CHANGED,
            byte_len: if enabled { 1 } else { 0 },
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::CanvasProfileChanged {
                new_profile_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn resolve_active_physics_profile(
        &self,
    ) -> crate::registries::atomic::lens::PhysicsProfileResolution {
        self.physics_profile
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active_profile()
    }

    fn set_active_physics_profile(
        &self,
        profile_id: &str,
    ) -> crate::registries::atomic::lens::PhysicsProfileResolution {
        let resolution = self
            .physics_profile
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_active_profile(profile_id);

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_PHYSICS_PROFILE_ACTIVATED,
            byte_len: resolution.resolved_id.len(),
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::PhysicsProfileChanged {
                new_profile_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn resolve_viewer_surface_profile(&self, viewer_id: &str) -> ViewerSurfaceResolution {
        let active_canvas = self.resolve_active_canvas_profile();
        let active_workbench = self.resolve_active_workbench_surface_profile();
        let viewer_capability = self.describe_viewer(viewer_id);
        let canvas_resolution = self
            .layout_domain
            .canvas()
            .resolve(&active_canvas.resolved_id);
        let workbench_resolution = self
            .layout_domain
            .workbench_surface()
            .resolve(&active_workbench.resolved_id);
        let viewer_surface = self
            .layout_domain
            .viewer_surface()
            .resolve_for_viewer(viewer_id, viewer_capability.as_ref());

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_LAYOUT_DOMAIN_PROFILE_RESOLVED,
            byte_len: viewer_surface.resolved_id.len(),
        });
        emit_surface_conformance_diagnostics(
            canvas_resolution.profile.subsystems.accessibility.level,
            canvas_resolution.profile.subsystems.security.level,
            canvas_resolution.profile.subsystems.storage.level,
            canvas_resolution.profile.subsystems.history.level,
        );
        emit_surface_conformance_diagnostics(
            workbench_resolution.profile.subsystems.accessibility.level,
            workbench_resolution.profile.subsystems.security.level,
            workbench_resolution.profile.subsystems.storage.level,
            workbench_resolution.profile.subsystems.history.level,
        );
        emit_surface_conformance_diagnostics(
            viewer_surface
                .profile
                .subsystems
                .accessibility
                .level
                .clone(),
            viewer_surface.profile.subsystems.security.level.clone(),
            viewer_surface.profile.subsystems.storage.level.clone(),
            viewer_surface.profile.subsystems.history.level.clone(),
        );
        viewer_surface
    }

    fn resolve_active_presentation_profile(
        &self,
        theme_id: Option<&str>,
    ) -> PresentationDomainProfileResolution {
        let physics = self.resolve_active_physics_profile();
        let theme = self.resolve_active_theme(theme_id);
        let mut resolution = self
            .presentation
            .resolve_profile(&physics.resolved_id, &theme.resolved_id);
        resolution.theme.requested_id = theme.requested_id;
        resolution.theme.resolved_id = theme.resolved_id;
        resolution.theme.matched = theme.matched;
        resolution.theme.fallback_used = theme.fallback_used;
        resolution.theme.theme_id = theme.tokens.theme_id.clone();
        resolution.theme.theme = theme.tokens.theme_data.clone();
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_PRESENTATION_PROFILE_RESOLVED,
            byte_len: resolution.resolved_profile_id.len(),
        });
        resolution
    }

    pub(crate) fn describe_workbench_surface(
        &self,
        profile_id: Option<&str>,
    ) -> WorkbenchSurfaceDescription {
        self.workbench_surface
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .describe_surface(profile_id)
    }

    fn resolve_active_workbench_surface_profile(&self) -> WorkbenchSurfaceResolution {
        self.workbench_surface
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active_profile()
    }

    fn set_active_workbench_surface_profile(&self, profile_id: &str) -> WorkbenchSurfaceResolution {
        let resolution = self
            .workbench_surface
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_active_profile(profile_id);

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_WORKBENCH_SURFACE_PROFILE_ACTIVATED,
            byte_len: resolution.resolved_id.len(),
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::WorkbenchSurfaceChanged {
                new_profile_id: resolution.resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolution
    }

    fn activate_workflow(
        &self,
        graph_app: &mut GraphBrowserApp,
        workflow_id: &str,
    ) -> Result<WorkflowActivation, WorkflowActivationError> {
        let resolution = self
            .workflow
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .resolve_workflow(Some(workflow_id));
        self.set_active_canvas_profile(&resolution.descriptor.canvas_profile);
        self.set_active_physics_profile(&resolution.descriptor.physics_profile);
        let workbench_profile = self
            .workbench_surface
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_active_profile(&resolution.descriptor.workbench_profile);
        let activation = self
            .workflow
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .activate(graph_app, workbench_profile.resolved_id.clone(), resolution)?;

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_WORKFLOW_ACTIVATED,
            byte_len: activation.workflow_id.len(),
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::WorkflowChanged {
                new_workflow_id: activation.workflow_id.clone(),
            }),
            SignalSource::ControlPanel,
            None,
        ));
        self.publish_signal(SignalEnvelope::new(
            SignalKind::Lifecycle(LifecycleSignal::WorkflowActivated {
                workflow_id: activation.workflow_id.clone(),
            }),
            SignalSource::ControlPanel,
            None,
        ));
        Ok(activation)
    }

    fn reconcile_semantics(&self, graph_app: &mut GraphBrowserApp) -> SemanticReconcileReport {
        let report = knowledge::reconcile_semantics(graph_app, &self.knowledge);
        if report.changed {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_KNOWLEDGE_INDEX_UPDATED,
                byte_len: report.indexed_nodes.max(1),
            });
            self.publish_signal(SignalEnvelope::new(
                SignalKind::RegistryEvent(RegistryEventSignal::SemanticIndexUpdated {
                    indexed_nodes: report.indexed_nodes,
                }),
                SignalSource::RegistryRuntime,
                None,
            ));
            self.publish_signal(SignalEnvelope::new(
                SignalKind::Lifecycle(LifecycleSignal::SemanticIndexUpdated {
                    indexed_nodes: report.indexed_nodes,
                }),
                SignalSource::RegistryRuntime,
                None,
            ));
        }
        report
    }

    pub(crate) fn query_knowledge_by_tag(&self, app: &GraphBrowserApp, tag: &str) -> Vec<NodeKey> {
        knowledge::query_by_tag(app, &self.knowledge, tag)
    }

    pub(crate) fn knowledge_tags_for_node(
        &self,
        app: &GraphBrowserApp,
        key: &NodeKey,
    ) -> Vec<String> {
        knowledge::tags_for_node(app, key)
    }

    pub(crate) fn validate_knowledge_tag(&self, tag: &str) -> TagValidationResult {
        let result = self.knowledge.validate_tag(tag);
        if matches!(
            result,
            TagValidationResult::Unknown { .. } | TagValidationResult::Malformed { .. }
        ) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_KNOWLEDGE_TAG_VALIDATION_WARN,
                byte_len: tag.trim().len().max(1),
            });
        }
        result
    }

    pub(crate) fn knowledge_label(&self, code: &str) -> Option<String> {
        self.knowledge.get_label(code).map(str::to_string)
    }

    pub(crate) fn knowledge_color_hint(&self, code: &str) -> Option<egui::Color32> {
        self.knowledge.get_color_hint(code)
    }

    pub(crate) fn suggest_knowledge_tags(&self, query: &str, limit: usize) -> Vec<String> {
        let mut suggestions = self
            .knowledge
            .search(query)
            .into_iter()
            .map(|entry| format!("udc:{}", entry.code))
            .collect::<Vec<_>>();
        suggestions.truncate(limit);
        suggestions
    }

    pub(crate) fn semantic_distance(&self, a: &str, b: &str) -> Option<f32> {
        self.knowledge.semantic_distance(a, b)
    }

    pub(crate) fn suggest_semantic_placement_anchor(
        &self,
        app: &GraphBrowserApp,
        key: NodeKey,
    ) -> Option<NodeKey> {
        knowledge::suggest_placement_anchor(app, &self.knowledge, key)
    }

    pub(crate) fn index_search(
        &self,
        app: &GraphBrowserApp,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let results = self
            .dynamic()
            .index
            .search(app, &self.knowledge, query, limit);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_INDEX_SEARCH,
            byte_len: query.len().saturating_add(results.len()),
        });
        results
    }

    pub(crate) fn route_agent_spawned(&self, agent_id: &str) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_AGENT_SPAWNED,
            byte_len: agent_id.len(),
        });
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::AgentSpawned {
                agent_id: agent_id.to_string(),
            }),
            SignalSource::ControlPanel,
            None,
        ));
    }

    fn dispatch_workbench_surface_intent(
        &self,
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &mut egui_tiles::Tree<crate::shell::desktop::workbench::tile_kind::TileKind>,
        intent: WorkbenchIntent,
    ) -> Option<WorkbenchIntent> {
        self.workbench_surface
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .dispatch_intent(graph_app, tiles_tree, intent)
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
        let protocol = match self.dynamic().protocol.resolve_with_control(uri, control) {
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
        let viewer = self
            .dynamic()
            .viewer
            .select_for_uri(uri, effective_mime_hint);
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_VIEWER_SELECT_SUCCEEDED,
            latency_us: 1,
        });

        self.publish_signal(SignalEnvelope::new(
            SignalKind::Navigation(NavigationSignal::Resolved {
                uri: uri.to_string(),
                viewer_id: viewer.viewer_id.to_string(),
            }),
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
        self.resolve_input_binding_resolution(resolution).is_some()
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
        let action_ids = remaps
            .iter()
            .filter_map(|remap| next_registry.resolve(&remap.new, remap.context).action_id)
            .collect::<Vec<_>>();
        *self
            .input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next_registry;

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_INPUT_BINDING_REBOUND,
            byte_len: remaps.len(),
        });
        for action_id in action_ids {
            self.publish_signal(SignalEnvelope::new(
                SignalKind::InputEvent(InputEventSignal::BindingRemapped { action_id }),
                SignalSource::RegistryRuntime,
                None,
            ));
        }

        Ok(())
    }

    pub(crate) fn reset_input_binding_remaps(&self) {
        *self
            .input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = InputRegistry::default();
        self.publish_signal(SignalEnvelope::new(
            SignalKind::InputEvent(InputEventSignal::BindingsReset),
            SignalSource::RegistryRuntime,
            None,
        ));
    }

    pub(crate) fn describe_input_bindings(&self) -> Vec<InputActionBindingDescriptor> {
        self.input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .describe_bindable_actions()
    }

    pub(crate) fn binding_display_labels_for_action(&self, action_id: &str) -> Vec<String> {
        self.input
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .binding_display_labels_for_action(action_id)
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

        let result = self
            .identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .sign(identity_id, payload);
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

    pub(crate) fn verify_identity_payload(
        &self,
        identity_id: &str,
        payload: &[u8],
        signature: &str,
    ) -> bool {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_IDENTITY_VERIFY_STARTED,
            byte_len: identity_id
                .len()
                .saturating_add(payload.len())
                .saturating_add(signature.len()),
        });

        let result = self
            .identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .verify(identity_id, payload, signature);

        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: if result.verified {
                CHANNEL_IDENTITY_VERIFY_SUCCEEDED
            } else {
                CHANNEL_IDENTITY_VERIFY_FAILED
            },
            latency_us: 1,
        });

        if !result.resolution.key_available {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_IDENTITY_KEY_UNAVAILABLE,
                byte_len: result.resolution.resolved_id.len(),
            });
        }

        result.verified
    }

    pub(crate) fn trusted_peers(&self) -> Vec<crate::mods::native::verse::TrustedPeer> {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .trusted_peers()
    }

    pub(crate) fn trusted_peers_handle(
        &self,
    ) -> std::sync::Arc<std::sync::RwLock<Vec<crate::mods::native::verse::TrustedPeer>>> {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .trusted_peers_handle()
    }

    pub(crate) fn trust_peer(&self, peer: crate::mods::native::verse::TrustedPeer) {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .trust_peer_record(peer);
    }

    pub(crate) fn revoke_peer(&self, node_id: iroh::EndpointId) {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .revoke_peer_record(node_id);
    }

    pub(crate) fn grant_workspace_access(
        &self,
        node_id: iroh::EndpointId,
        workspace_id: &str,
        access: crate::mods::native::verse::AccessLevel,
    ) {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .grant_workspace_access(node_id, workspace_id, access);
    }

    pub(crate) fn revoke_workspace_access(&self, node_id: iroh::EndpointId, workspace_id: &str) {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .revoke_workspace_access(node_id, workspace_id);
    }

    pub(crate) fn create_presence_binding_assertion(
        &self,
        audience: &str,
        ttl_secs: u64,
    ) -> Option<PresenceBindingAssertion> {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .create_presence_binding_assertion("identity:default", audience, ttl_secs)
    }

    pub(crate) fn verify_presence_binding_assertion(
        &self,
        assertion: &PresenceBindingAssertion,
    ) -> bool {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .verify_presence_binding_assertion(assertion)
    }

    pub(crate) fn subscribe_signal(
        &self,
        topic: SignalTopic,
        callback: impl Fn(&SignalEnvelope) -> Result<(), String> + Send + Sync + 'static,
    ) -> ObserverId {
        self.signal_bus.subscribe_sync(topic, Arc::new(callback))
    }

    pub(crate) fn unsubscribe_signal(&self, topic: SignalTopic, observer_id: ObserverId) -> bool {
        self.signal_bus.unsubscribe(topic, observer_id)
    }

    pub(crate) fn subscribe_signal_async(&self, topic: SignalTopic) -> AsyncSignalSubscription {
        self.signal_bus.subscribe_async(topic)
    }

    pub(crate) fn subscribe_all_signals_async(&self) -> AsyncSignalSubscription {
        self.signal_bus.subscribe_all()
    }

    pub(crate) fn signal_trace_snapshot(&self) -> Vec<signal_routing::SignalTraceEntry> {
        self.signal_bus.signal_trace()
    }

    #[cfg(test)]
    fn signal_routing_diagnostics(&self) -> signal_routing::SignalRoutingDiagnostics {
        self.signal_bus.diagnostics()
    }

    #[cfg(test)]
    fn signal_routing_dead_letters(&self) -> Vec<signal_routing::SignalDeadLetter> {
        self.signal_bus.dead_letters()
    }

    fn publish_signal(&self, envelope: SignalEnvelope) {
        let report = self.signal_bus.publish(envelope);
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
                channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_FAILED,
                byte_len: report.observer_failures,
            });
        }

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_QUEUE_DEPTH,
            byte_len: report.queue_depth,
        });
    }

    pub(crate) fn route_mod_lifecycle_event(&self, mod_id: &str, activated: bool) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(if activated {
                RegistryEventSignal::ModLoaded {
                    mod_id: mod_id.to_string(),
                }
            } else {
                RegistryEventSignal::ModUnloaded {
                    mod_id: mod_id.to_string(),
                }
            }),
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
            SignalKind::Lifecycle(LifecycleSignal::MemoryPressureChanged {
                level: level_name.to_string(),
                available_mib,
                total_mib,
            }),
            SignalSource::ControlPanel,
            None,
        ));

        let byte_len = (available_mib as usize).saturating_add(total_mib as usize);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_REGISTER_SIGNAL_ROUTING_SUBSYSTEM_HEALTH_PROPAGATED,
            byte_len,
        });
    }

    pub(crate) fn publish_navigation_mime_resolved(
        &self,
        key: NodeKey,
        uri: &str,
        mime_hint: Option<&str>,
    ) {
        let viewer = self.dynamic().viewer.select_for_uri(uri, mime_hint);
        self.publish_signal(SignalEnvelope::new(
            SignalKind::Navigation(NavigationSignal::MimeResolved {
                key,
                uri: uri.to_string(),
                mime_hint: mime_hint.map(str::to_string),
                viewer_id: viewer.viewer_id.to_string(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        if let Some(mime) = mime_hint {
            self.publish_signal(SignalEnvelope::new(
                SignalKind::Lifecycle(LifecycleSignal::MimeResolved {
                    node_key: key,
                    mime: mime.to_string(),
                }),
                SignalSource::RegistryRuntime,
                None,
            ));
        }
    }

    pub(crate) fn publish_navigation_node_activated(&self, key: NodeKey, uri: &str, title: &str) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::Navigation(NavigationSignal::NodeActivated {
                key,
                uri: uri.to_string(),
                title: title.to_string(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
    }

    pub(crate) fn publish_workbench_projection_refresh_requested(&self, reason: &str) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::WorkbenchProjectionRefreshRequested {
                reason: reason.to_string(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
    }

    pub(crate) fn publish_settings_route_requested(&self, url: &str) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::SettingsRouteRequested {
                url: url.to_string(),
            }),
            SignalSource::ControlPanel,
            None,
        ));
    }

    pub(crate) fn publish_lens_changed(&self, lens_id: Option<&str>) -> String {
        let requested = lens_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(lens::LENS_ID_DEFAULT);
        let resolved_id = self.dynamic().lens.resolve(requested).resolved_id;
        self.publish_signal(SignalEnvelope::new(
            SignalKind::RegistryEvent(RegistryEventSignal::LensChanged {
                new_lens_id: resolved_id.clone(),
            }),
            SignalSource::RegistryRuntime,
            None,
        ));
        resolved_id
    }

    /// Emit a `LifecycleSignal::UserIdle` signal through the signal bus.
    ///
    /// Called by `ControlPanel::tick_idle_watchdog` when the idle threshold
    /// is crossed.
    pub(crate) fn propagate_user_idle_signal(&self, since_ms: u64) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::Lifecycle(LifecycleSignal::UserIdle { since_ms }),
            SignalSource::ControlPanel,
            None,
        ));
    }

    /// Emit a `LifecycleSignal::UserResumed` signal through the signal bus.
    ///
    /// Called by `ControlPanel::tick_idle_watchdog` when the user returns
    /// from an idle period.
    pub(crate) fn propagate_user_resumed_signal(&self) {
        self.publish_signal(SignalEnvelope::new(
            SignalKind::Lifecycle(LifecycleSignal::UserResumed),
            SignalSource::ControlPanel,
            None,
        ));
    }

    #[cfg(test)]
    pub(crate) fn publish_signal_for_tests(&self, envelope: SignalEnvelope) {
        self.publish_signal(envelope);
    }
}

pub(crate) fn phase2_resolve_toolbar_submit_binding() -> bool {
    phase2_resolve_input_binding(input::binding_id::toolbar::SUBMIT)
}

pub(crate) fn phase0_select_viewer_for_content(
    uri: &str,
    mime_hint: Option<&str>,
) -> ViewerSelection {
    runtime().select_viewer_for_content(uri, mime_hint)
}

pub(crate) fn phase0_describe_viewer(viewer_id: &str) -> Option<ViewerCapability> {
    runtime().describe_viewer(viewer_id)
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

pub(crate) fn phase3_publish_navigation_mime_resolved(
    key: NodeKey,
    uri: &str,
    mime_hint: Option<&str>,
) {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().publish_navigation_mime_resolved(key, uri, mime_hint);
}

pub(crate) fn phase3_publish_navigation_node_activated(key: NodeKey, uri: &str, title: &str) {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().publish_navigation_node_activated(key, uri, title);
}

pub(crate) fn phase3_publish_workbench_projection_refresh_requested(reason: &str) {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().publish_workbench_projection_refresh_requested(reason);
}

pub(crate) fn phase3_publish_settings_route_requested(url: &str) {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().publish_settings_route_requested(url);
}

pub(crate) fn phase3_publish_lens_changed(lens_id: Option<&str>) -> String {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().publish_lens_changed(lens_id)
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

pub(crate) fn phase2_describe_input_bindings() -> Vec<InputActionBindingDescriptor> {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());
    runtime().describe_input_bindings()
}

pub(crate) fn phase2_binding_display_labels_for_action(action_id: &str) -> Vec<String> {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());
    runtime().binding_display_labels_for_action(action_id)
}

pub(crate) fn phase2_resolve_lens(lens_id: &str) -> crate::app::ResolvedLensPreset {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    let runtime = runtime();
    let resolution = runtime.dynamic().lens.resolve(lens_id);
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

    crate::app::ResolvedLensPreset {
        lens_id: resolution.resolved_id,
        display_name: resolution.definition.display_name,
        physics: resolution.definition.physics,
        layout: resolution.definition.layout,
        layout_algorithm_id: resolution.definition.layout_algorithm_id,
        theme: resolution.definition.theme,
        filter_expr: None,
        filters_legacy: resolution.definition.filters,
        overlay_descriptor: resolution.definition.overlay_descriptor,
    }
}

pub(crate) fn phase2_resolve_lens_for_content(
    mime_hint: Option<&str>,
    has_semantic_context: bool,
) -> crate::app::ResolvedLensPreset {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    let runtime = runtime();
    let lens_ids = runtime
        .dynamic()
        .lens
        .resolve_for_content(mime_hint, has_semantic_context);
    let primary_id = lens_ids.first().cloned().unwrap_or_else(|| {
        crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT.to_string()
    });
    let composed = runtime.dynamic().lens.compose(&lens_ids);

    crate::app::ResolvedLensPreset {
        lens_id: primary_id,
        display_name: composed.display_name,
        physics: composed.physics,
        layout: composed.layout,
        layout_algorithm_id: composed.layout_algorithm_id,
        theme: composed.theme,
        filter_expr: None,
        filters_legacy: composed.filters,
        overlay_descriptor: composed.overlay_descriptor,
    }
}

pub(crate) fn phase2_resolve_lens_for_node(
    app: &GraphBrowserApp,
    key: NodeKey,
) -> crate::app::ResolvedLensPreset {
    let Some(node) = app.domain_graph().get_node(key) else {
        return phase2_resolve_lens(
            crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT,
        );
    };
    let has_semantic_context = !runtime().knowledge_tags_for_node(app, &key).is_empty();
    phase2_resolve_lens_for_content(node.mime_hint.as_deref(), has_semantic_context)
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

    // Use execute_action (not dynamic().action.execute) to avoid holding the
    // dynamic mutex during the handler — the omnibox handler calls back into
    // runtime().dynamic() via phase3_index_search, which would deadlock.
    let execution = runtime().execute_action(
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
            .dynamic()
            .action
            .execute(ACTION_VERSE_SYNC_NOW, app, ActionPayload::VerseSyncNow);
    execution.into_intents()
}

pub(crate) fn phase5_execute_verse_pair_local_peer_action(
    app: &GraphBrowserApp,
    node_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().dynamic().action.execute(
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
    let execution = runtime().dynamic().action.execute(
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
    let execution = runtime().dynamic().action.execute(
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
    let execution = runtime().dynamic().action.execute(
        ACTION_VERSE_FORGET_DEVICE,
        app,
        ActionPayload::VerseForgetDevice {
            node_id: node_id.to_string(),
        },
    );
    execution.into_intents()
}

pub(crate) fn describe_action_capability(action_id: &str) -> Option<ActionCapability> {
    runtime().describe_action(action_id)
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

    let result = runtime()
        .identity
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
pub(crate) fn phase3_verify_identity_payload_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    identity_id: &str,
    payload: &[u8],
    signature: &str,
) -> bool {
    diagnostics_state.emit_message_sent_for_tests(
        CHANNEL_IDENTITY_VERIFY_STARTED,
        identity_id
            .len()
            .saturating_add(payload.len())
            .saturating_add(signature.len()),
    );

    let result = runtime()
        .identity
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .verify(identity_id, payload, signature);

    diagnostics_state.emit_message_received_for_tests(
        if result.verified {
            CHANNEL_IDENTITY_VERIFY_SUCCEEDED
        } else {
            CHANNEL_IDENTITY_VERIFY_FAILED
        },
        1,
    );
    if !result.resolution.key_available {
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_IDENTITY_KEY_UNAVAILABLE,
            result.resolution.resolved_id.len(),
        );
    }

    result.verified
}

#[cfg(test)]
pub(crate) fn phase2_resolve_toolbar_submit_binding_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) -> bool {
    phase2_resolve_input_binding_for_tests(diagnostics_state, input::binding_id::toolbar::SUBMIT)
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

    diagnostics_state.emit_message_sent_for_tests(
        CHANNEL_INPUT_BINDING_MISSING,
        resolution.binding_label.len(),
    );
    false
}

#[cfg(test)]
pub(crate) fn phase2_resolve_lens_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    lens_id: &str,
) -> crate::app::ResolvedLensPreset {
    let runtime = RegistryRuntime::default();
    let resolution = runtime.dynamic().lens.resolve(lens_id);

    if resolution.matched {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_LENS_RESOLVE_SUCCEEDED, 1);
    } else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_LENS_RESOLVE_FAILED, 1);
    }

    if resolution.fallback_used {
        diagnostics_state
            .emit_message_sent_for_tests(CHANNEL_LENS_FALLBACK_USED, resolution.resolved_id.len());
    }

    crate::app::ResolvedLensPreset {
        lens_id: resolution.resolved_id,
        display_name: resolution.definition.display_name,
        physics: resolution.definition.physics,
        layout: resolution.definition.layout,
        layout_algorithm_id: resolution.definition.layout_algorithm_id,
        theme: resolution.definition.theme,
        filter_expr: None,
        filters_legacy: resolution.definition.filters,
        overlay_descriptor: resolution.definition.overlay_descriptor,
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

    let execution = runtime().dynamic().action.execute(
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

    let execution = runtime().dynamic().action.execute(
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

    let (mutations, runtime_events) =
        split_detail_submit_intents(intents, ACTION_DETAIL_VIEW_SUBMIT);
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
    runtime().resolve_viewer_surface_profile(_viewer_id)
}

pub(crate) fn phase3_describe_layout_algorithm(
    algorithm_id: Option<&str>,
) -> crate::app::graph_layout::LayoutCapability {
    runtime().describe_layout_algorithm(algorithm_id)
}

pub(crate) fn phase3_resolve_layout_algorithm(
    algorithm_id: Option<&str>,
) -> crate::app::graph_layout::LayoutResolution {
    runtime().resolve_layout_algorithm(algorithm_id)
}

pub(crate) fn phase3_apply_layout_algorithm_to_graph(
    graph: &mut crate::graph::Graph,
    algorithm_id: Option<&str>,
) -> Result<crate::app::graph_layout::LayoutExecution, String> {
    runtime().apply_layout_algorithm_to_graph(graph, algorithm_id)
}

pub(crate) fn phase3_resolve_active_workbench_surface_profile() -> WorkbenchSurfaceResolution {
    runtime().resolve_active_workbench_surface_profile()
}

pub(crate) fn phase3_resolve_active_canvas_profile() -> CanvasSurfaceResolution {
    runtime().resolve_active_canvas_profile()
}

pub(crate) fn phase3_set_active_canvas_profile(profile_id: &str) -> CanvasSurfaceResolution {
    runtime().set_active_canvas_profile(profile_id)
}

pub(crate) fn phase3_set_active_canvas_lasso_binding(
    binding: CanvasLassoBinding,
) -> CanvasSurfaceResolution {
    runtime().set_active_canvas_lasso_binding(binding)
}

pub(crate) fn phase3_set_active_canvas_keyboard_pan_step(step: f32) -> CanvasSurfaceResolution {
    runtime().set_active_canvas_keyboard_pan_step(step)
}

/// Enable or disable frame-affinity backdrop rendering and soft centroid-attraction force.
///
/// Spec: `layout_behaviors_and_physics_spec.md §4.3`
pub(crate) fn phase3_set_canvas_frame_affinity_enabled(enabled: bool) -> CanvasSurfaceResolution {
    runtime().set_canvas_frame_affinity_enabled(enabled)
}

pub(crate) fn phase3_resolve_active_physics_profile()
-> crate::registries::atomic::lens::PhysicsProfileResolution {
    runtime().resolve_active_physics_profile()
}

pub(crate) fn phase3_set_active_physics_profile(
    profile_id: &str,
) -> crate::registries::atomic::lens::PhysicsProfileResolution {
    runtime().set_active_physics_profile(profile_id)
}

pub(crate) fn phase3_set_active_theme(theme_id: &str) -> ThemeResolution {
    runtime().set_active_theme(theme_id)
}

pub(crate) fn phase3_resolve_active_theme(theme_id: Option<&str>) -> ThemeResolution {
    runtime().resolve_active_theme(theme_id)
}

pub(crate) fn phase3_apply_system_theme_preference(prefers_dark: bool) -> Option<ThemeResolution> {
    runtime().apply_system_theme_preference(prefers_dark)
}

pub(crate) fn phase3_set_theme_follows_system(follows: bool) {
    runtime().set_theme_follows_system(follows);
}

pub(crate) fn phase3_describe_theme(theme_id: Option<&str>) -> ThemeCapability {
    runtime().describe_theme(theme_id)
}

pub(crate) fn phase3_resolve_active_presentation_profile(
    theme_id: Option<&str>,
) -> PresentationDomainProfileResolution {
    runtime().resolve_active_presentation_profile(theme_id)
}

pub(crate) fn phase3_set_active_workbench_surface_profile(
    profile_id: &str,
) -> WorkbenchSurfaceResolution {
    runtime().set_active_workbench_surface_profile(profile_id)
}

pub(crate) fn phase3_describe_workbench_surface(
    profile_id: Option<&str>,
) -> WorkbenchSurfaceDescription {
    runtime().describe_workbench_surface(profile_id)
}

pub(crate) fn phase3_describe_workflow(workflow_id: Option<&str>) -> WorkflowCapability {
    runtime().describe_workflow(workflow_id)
}

pub(crate) fn phase3_activate_workflow(
    graph_app: &mut GraphBrowserApp,
    workflow_id: &str,
) -> Result<WorkflowActivation, WorkflowActivationError> {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());
    runtime().activate_workflow(graph_app, workflow_id)
}

pub(crate) fn phase3_apply_runtime_action_dispatch(
    graph_app: &mut GraphBrowserApp,
    dispatch: ActionDispatch,
) -> Result<ActionDispatch, ActionFailure> {
    for action in &dispatch.runtime_actions {
        match action {
            RuntimeAction::ActivateWorkflow { workflow_id } => {
                runtime()
                    .activate_workflow(graph_app, workflow_id)
                    .map_err(|error| ActionFailure {
                        kind: action::ActionFailureKind::Rejected,
                        reason: match error {
                            WorkflowActivationError::NotImplemented { workflow_id } => {
                                format!("workflow '{workflow_id}' is not implemented")
                            }
                        },
                    })?;
            }
            RuntimeAction::PublishSettingsRouteRequested {
                url,
            } => {
                runtime().publish_settings_route_requested(url);
            }
        }
    }

    Ok(dispatch)
}

pub(crate) fn phase3_execute_registry_action(
    graph_app: &mut GraphBrowserApp,
    action_id: &str,
    payload: ActionPayload,
) -> Result<Vec<GraphIntent>, ActionFailure> {
    let execution = runtime()
        .dynamic()
        .action
        .execute(action_id, graph_app, payload);
    let dispatch = match execution {
        ActionOutcome::Dispatch(dispatch) => dispatch,
        ActionOutcome::Failure(failure) => return Err(failure),
    };
    let dispatch = phase3_apply_runtime_action_dispatch(graph_app, dispatch)?;

    for intent in dispatch.workbench_intents {
        graph_app.enqueue_workbench_intent(intent);
    }
    for command in dispatch.app_commands {
        graph_app.enqueue_app_command(command);
    }

    Ok(dispatch.intents)
}

pub(crate) fn phase3_reconcile_semantics(
    graph_app: &mut GraphBrowserApp,
) -> SemanticReconcileReport {
    runtime().reconcile_semantics(graph_app)
}

pub(crate) fn phase3_validate_knowledge_tag(tag: &str) -> TagValidationResult {
    runtime().validate_knowledge_tag(tag)
}

pub(crate) fn phase3_query_knowledge_by_tag(app: &GraphBrowserApp, tag: &str) -> Vec<NodeKey> {
    runtime().query_knowledge_by_tag(app, tag)
}

pub(crate) fn phase3_knowledge_tags_for_node(app: &GraphBrowserApp, key: &NodeKey) -> Vec<String> {
    runtime().knowledge_tags_for_node(app, key)
}

pub(crate) fn phase3_knowledge_label(code: &str) -> Option<String> {
    runtime().knowledge_label(code)
}

pub(crate) fn phase3_knowledge_color_hint(code: &str) -> Option<egui::Color32> {
    runtime().knowledge_color_hint(code)
}

pub(crate) fn phase3_semantic_distance(a: &str, b: &str) -> Option<f32> {
    runtime().semantic_distance(a, b)
}

pub(crate) fn phase3_suggest_semantic_placement_anchor(
    app: &GraphBrowserApp,
    key: NodeKey,
) -> Option<NodeKey> {
    runtime().suggest_semantic_placement_anchor(app, key)
}

pub(crate) fn phase3_index_search(
    app: &GraphBrowserApp,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    runtime().index_search(app, query, limit)
}

pub(crate) fn phase3_subscribe_signal(
    topic: SignalTopic,
    callback: impl Fn(&SignalEnvelope) -> Result<(), String> + Send + Sync + 'static,
) -> ObserverId {
    runtime().subscribe_signal(topic, callback)
}

pub(crate) fn phase3_unsubscribe_signal(topic: SignalTopic, observer_id: ObserverId) -> bool {
    runtime().unsubscribe_signal(topic, observer_id)
}

pub(crate) fn phase3_subscribe_signal_async(topic: SignalTopic) -> AsyncSignalSubscription {
    runtime().subscribe_signal_async(topic)
}

pub(crate) fn phase3_subscribe_all_signals_async() -> AsyncSignalSubscription {
    runtime().subscribe_all_signals_async()
}

pub(crate) fn phase3_signal_trace_snapshot() -> Vec<signal_routing::SignalTraceEntry> {
    runtime().signal_trace_snapshot()
}

pub(crate) fn phase3_shared_runtime() -> Arc<RegistryRuntime> {
    shared_runtime()
}

pub(crate) fn dispatch_workbench_surface_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut egui_tiles::Tree<crate::shell::desktop::workbench::tile_kind::TileKind>,
    intent: WorkbenchIntent,
) -> Option<WorkbenchIntent> {
    runtime().dispatch_workbench_surface_intent(graph_app, tiles_tree, intent)
}

// ---------------------------------------------------------------------------
// Gemini capsule server
// ---------------------------------------------------------------------------

/// Start the Gemini capsule server on `port`.
///
/// If a server is already running it is stopped first.
pub(crate) fn start_gemini_capsule_server(port: u16) {
    // Stop any existing server.
    stop_gemini_capsule_server();

    let registry = gemini_registry().clone();
    let config = crate::mods::native::verso::gemini::GeminiServerConfig {
        port,
        hostname: System::host_name().unwrap_or_else(|| "localhost".to_string()),
    };

    let server = crate::mods::native::verso::gemini::GeminiCapsuleServer::new_with_registry(
        config, registry,
    );

    let runtime = tokio::runtime::Handle::try_current();
    match runtime {
        Ok(handle) => {
            let result = handle.block_on(server.start());
            match result {
                Ok(server_handle) => {
                    log::info!(
                        "gemini: capsule server started on {}",
                        server_handle.bound_addr
                    );
                    *GEMINI_SERVER_HANDLE.lock().unwrap() = Some(server_handle);
                }
                Err(e) => {
                    log::warn!("gemini: failed to start capsule server: {e}");
                }
            }
        }
        Err(_) => {
            log::warn!("gemini: no tokio runtime available; cannot start capsule server");
        }
    }
}

/// Stop the running Gemini capsule server, if any.
pub(crate) fn stop_gemini_capsule_server() {
    if let Some(handle) = GEMINI_SERVER_HANDLE.lock().unwrap().take() {
        handle.stop();
        log::info!("gemini: capsule server stopped");
    }
}

/// Register a node for serving via the capsule server.
pub(crate) fn register_gemini_node(
    node_id: uuid::Uuid,
    title: String,
    privacy_class: crate::model::archive::ArchivePrivacyClass,
    gemini_content: String,
) {
    gemini_registry().register(crate::mods::native::verso::gemini::ServedNode {
        node_id,
        title,
        privacy_class,
        gemini_content,
    });
}

/// Remove a node from the Gemini capsule server registry.
pub(crate) fn unregister_gemini_node(node_id: uuid::Uuid) {
    gemini_registry().unregister(node_id);
}

// ---------------------------------------------------------------------------
// Gopher capsule server
// ---------------------------------------------------------------------------

pub(crate) fn start_gopher_capsule_server(port: u16) {
    stop_gopher_capsule_server();

    let registry = gopher_registry().clone();
    let config = crate::mods::native::verso::gopher::GopherServerConfig {
        port,
        hostname: System::host_name().unwrap_or_else(|| "localhost".to_string()),
    };
    let server = crate::mods::native::verso::gopher::GopherCapsuleServer::new_with_registry(
        config, registry,
    );
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.block_on(server.start()) {
            Ok(server_handle) => {
                log::info!(
                    "gopher: capsule server started on {}",
                    server_handle.bound_addr
                );
                *GOPHER_SERVER_HANDLE.lock().unwrap() = Some(server_handle);
            }
            Err(e) => log::warn!("gopher: failed to start capsule server: {e}"),
        },
        Err(_) => log::warn!("gopher: no tokio runtime available"),
    }
}

pub(crate) fn stop_gopher_capsule_server() {
    if let Some(handle) = GOPHER_SERVER_HANDLE.lock().unwrap().take() {
        handle.stop();
        log::info!("gopher: capsule server stopped");
    }
}

pub(crate) fn register_gopher_node(
    node_id: uuid::Uuid,
    title: String,
    privacy_class: crate::model::archive::ArchivePrivacyClass,
    gophermap_content: String,
) {
    gopher_registry().register(crate::mods::native::verso::gopher::GopherServedNode {
        node_id,
        title,
        privacy_class,
        gophermap_content,
    });
}

pub(crate) fn unregister_gopher_node(node_id: uuid::Uuid) {
    gopher_registry().unregister(node_id);
}

// ---------------------------------------------------------------------------
// Finger server
// ---------------------------------------------------------------------------

pub(crate) fn start_finger_server(port: u16) {
    stop_finger_server();

    let registry = finger_registry().clone();
    let config = crate::mods::native::verso::finger::FingerServerConfig {
        port,
        default_query: "graphshell".to_string(),
    };
    let server =
        crate::mods::native::verso::finger::FingerServer::new_with_registry(config, registry);
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.block_on(server.start()) {
            Ok(server_handle) => {
                log::info!("finger: server started on {}", server_handle.bound_addr);
                *FINGER_SERVER_HANDLE.lock().unwrap() = Some(server_handle);
            }
            Err(e) => log::warn!("finger: failed to start server: {e}"),
        },
        Err(_) => log::warn!("finger: no tokio runtime available"),
    }
}

pub(crate) fn stop_finger_server() {
    if let Some(handle) = FINGER_SERVER_HANDLE.lock().unwrap().take() {
        handle.stop();
        log::info!("finger: server stopped");
    }
}

pub(crate) fn publish_finger_profile(
    query_name: String,
    privacy_class: crate::model::archive::ArchivePrivacyClass,
    finger_text: String,
) {
    finger_registry().register(crate::mods::native::verso::finger::FingerProfile {
        query_name,
        privacy_class,
        finger_text,
    });
}

pub(crate) fn unpublish_finger_profile(query_name: String) {
    finger_registry().unregister(&query_name);
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
    let protocol = match runtime
        .dynamic()
        .protocol
        .resolve_with_control(uri, control)
    {
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
    let viewer = runtime
        .dynamic()
        .viewer
        .select_for_uri(uri, effective_mime_hint);
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

    let execution = RegistryRuntime::default().dynamic().action.execute(
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

    let execution = RegistryRuntime::default().dynamic().action.execute(
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

    let execution = RegistryRuntime::default().dynamic().action.execute(
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

    let (mutations, runtime_events) =
        split_detail_submit_intents(intents, ACTION_DETAIL_VIEW_SUBMIT);
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
    peer_id: iroh::EndpointId,
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

    #[test]
    fn registry_runtime_describes_action_capabilities() {
        let runtime = RegistryRuntime::default();

        assert_eq!(
            runtime.describe_action(action::ACTION_GRAPH_DESELECT_ALL),
            Some(ActionCapability::RequiresSelection)
        );
        assert_eq!(
            describe_action_capability(action::ACTION_WORKBENCH_SETTINGS_OPEN),
            Some(ActionCapability::AlwaysAvailable)
        );
        assert_eq!(
            describe_action_capability(action::ACTION_WORKBENCH_SETTINGS_PANE_OPEN),
            Some(ActionCapability::AlwaysAvailable)
        );
        assert_eq!(
            describe_action_capability(action::ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN),
            Some(ActionCapability::AlwaysAvailable)
        );
    }

    #[test]
    fn registry_runtime_describes_viewer_capabilities() {
        let runtime = RegistryRuntime::default();
        let capability = runtime
            .describe_viewer("viewer:webview")
            .expect("viewer:webview should be described");

        assert_eq!(capability.viewer_id, "viewer:webview");
        assert!(
            capability
                .supported_mime_types
                .iter()
                .any(|mime| mime == "text/html")
        );
    }

    #[test]
    fn publish_navigation_mime_resolved_routes_navigation_signal() {
        let runtime = RegistryRuntime::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        runtime.subscribe_signal(SignalTopic::Navigation, move |signal| {
            seen.lock()
                .expect("observer lock poisoned")
                .push(signal.kind.clone());
            Ok(())
        });

        runtime.publish_navigation_mime_resolved(
            NodeKey::new(17),
            "https://example.com/data.csv",
            Some("text/csv"),
        );

        let observed = observed.lock().expect("observer lock poisoned");
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::Navigation(NavigationSignal::MimeResolved {
                key,
                mime_hint,
                viewer_id,
                ..
            }) if *key == NodeKey::new(17)
                && mime_hint.as_deref() == Some("text/csv")
                && viewer_id == "viewer:csv"
        )));
    }

    #[test]
    fn publish_navigation_mime_resolved_routes_lifecycle_signal() {
        let runtime = RegistryRuntime::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
            seen.lock()
                .expect("observer lock poisoned")
                .push(signal.kind.clone());
            Ok(())
        });

        runtime.publish_navigation_mime_resolved(
            NodeKey::new(23),
            "https://example.com/data.csv",
            Some("text/csv"),
        );

        let observed = observed.lock().expect("observer lock poisoned");
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::Lifecycle(LifecycleSignal::MimeResolved { node_key, mime })
                if *node_key == NodeKey::new(23) && mime == "text/csv"
        )));
    }

    #[test]
    fn set_active_physics_profile_routes_registry_event_signal() {
        let runtime = RegistryRuntime::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        runtime.subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
            seen.lock()
                .expect("observer lock poisoned")
                .push(signal.kind.clone());
            Ok(())
        });

        let resolution =
            runtime.set_active_physics_profile(physics_profile::PHYSICS_PROFILE_SETTLE);
        assert_eq!(
            resolution.resolved_id,
            physics_profile::PHYSICS_PROFILE_SETTLE
        );

        let observed = observed.lock().expect("observer lock poisoned");
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::RegistryEvent(RegistryEventSignal::PhysicsProfileChanged {
                new_profile_id,
            }) if new_profile_id == physics_profile::PHYSICS_PROFILE_SETTLE
        )));
    }

    #[test]
    fn apply_input_binding_remaps_routes_input_event_signal() {
        let runtime = RegistryRuntime::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        runtime.subscribe_signal(SignalTopic::InputEvent, move |signal| {
            seen.lock()
                .expect("observer lock poisoned")
                .push(signal.kind.clone());
            Ok(())
        });

        let remap = InputBindingRemap {
            old: "gamepad:left_bumper"
                .parse::<InputBinding>()
                .expect("old binding should parse"),
            new: "gamepad:left_bumper+east"
                .parse::<InputBinding>()
                .expect("new binding should parse"),
            context: InputContext::DetailView,
        };
        runtime
            .apply_input_binding_remaps(&[remap])
            .expect("remap should succeed");
        runtime.reset_input_binding_remaps();

        let observed = observed.lock().expect("observer lock poisoned");
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::InputEvent(InputEventSignal::BindingRemapped { action_id })
                if action_id == crate::shell::desktop::runtime::registries::input::action_id::toolbar::NAV_BACK
        )));
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::InputEvent(InputEventSignal::BindingsReset)
        )));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn phase3_async_signal_subscription_receives_runtime_publish() {
        let runtime = RegistryRuntime::default();
        let mut receiver = runtime.subscribe_signal_async(SignalTopic::Lifecycle);

        runtime.publish_navigation_mime_resolved(
            NodeKey::new(41),
            "https://example.com/data.csv",
            Some("text/csv"),
        );

        let received = receiver
            .recv()
            .await
            .expect("async runtime receiver should stay open");
        assert!(matches!(
            received.kind,
            SignalKind::Lifecycle(LifecycleSignal::MimeResolved { node_key, ref mime })
                if node_key == NodeKey::new(41) && mime == "text/csv"
        ));
    }

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

    #[cfg(feature = "wry")]
    #[test]
    fn new_with_mods_registers_wry_viewer_capability_as_native_overlay() {
        use crate::registries::atomic::viewer::ViewerRenderMode;
        let runtime = RegistryRuntime::new_with_mods();
        let capability = runtime
            .describe_viewer("viewer:wry")
            .expect("viewer:wry should be described after verso mod activation");
        assert_eq!(capability.render_mode, ViewerRenderMode::NativeOverlay);
        assert_eq!(capability.viewer_id, "viewer:wry");
    }

    #[test]
    #[cfg(feature = "pdf")]
    fn runtime_owned_mod_registry_can_unload_verso_and_restore_pdf_viewer_mapping() {
        let runtime = RegistryRuntime::new_with_mods();
        let before = runtime
            .observe_navigation_url_with_control(
                "https://example.com/reference.pdf",
                Some("application/pdf"),
                ProtocolResolveControl::default(),
            )
            .expect("verso-backed runtime should resolve navigation")
            .1;
        assert_eq!(before.viewer_id, "viewer:webview");

        let extension_count = runtime
            .mod_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .extension_records_for("mod:verso")
            .map(|records| records.len())
            .unwrap_or_default();
        assert!(extension_count > 0);

        runtime
            .unload_mod("mod:verso")
            .expect("verso should unload cleanly");

        let after = runtime
            .observe_navigation_url_with_control(
                "https://example.com/reference.pdf",
                Some("application/pdf"),
                ProtocolResolveControl::default(),
            )
            .expect("runtime should still resolve after unload")
            .1;
        assert_eq!(after.viewer_id, "viewer:pdf");
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
        assert_eq!(diagnostics.unrouted_signals, 1);
        assert_eq!(runtime.signal_routing_dead_letters().len(), 1);
    }

    #[test]
    fn phase2_input_binding_path_matches_registry_runtime_dispatch_api() {
        let via_phase_api = phase2_resolve_input_binding(input::binding_id::toolbar::SUBMIT);
        let via_runtime_api = runtime().resolve_input_binding(input::binding_id::toolbar::SUBMIT);
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
    fn phase3_mod_workflow_path_routes_through_registry_event_signal_observers() {
        let runtime = RegistryRuntime::default();
        let observer_a = Arc::new(AtomicUsize::new(0));
        let observer_b = Arc::new(AtomicUsize::new(0));

        {
            let observer_a = Arc::clone(&observer_a);
            runtime.subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
                if matches!(
                    signal.kind,
                    SignalKind::RegistryEvent(RegistryEventSignal::ModLoaded { .. })
                ) {
                    observer_a.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        {
            let observer_b = Arc::clone(&observer_b);
            runtime.subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
                if let SignalKind::RegistryEvent(RegistryEventSignal::ModLoaded { mod_id }) =
                    &signal.kind
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
    fn phase3_settings_route_requested_routes_through_registry_event_signal_observers() {
        let runtime = RegistryRuntime::default();
        let observer_count = Arc::new(AtomicUsize::new(0));

        {
            let observer_count = Arc::clone(&observer_count);
            runtime.subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
                if let SignalKind::RegistryEvent(RegistryEventSignal::SettingsRouteRequested {
                    url,
                }) = &signal.kind
                    && url == "verso://settings/general"
                {
                    observer_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        runtime.publish_settings_route_requested("verso://settings/general");

        assert_eq!(observer_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn publish_lens_changed_routes_registry_event_signal_with_resolved_lens_id() {
        let runtime = RegistryRuntime::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        runtime.subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
            seen.lock()
                .expect("observer lock poisoned")
                .push(signal.kind.clone());
            Ok(())
        });

        let resolved_id = runtime.publish_lens_changed(Some("lens:unknown"));

        assert_eq!(resolved_id, lens::LENS_ID_DEFAULT);
        let observed = observed.lock().expect("observer lock poisoned");
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::RegistryEvent(RegistryEventSignal::LensChanged { new_lens_id })
                if new_lens_id == lens::LENS_ID_DEFAULT
        )));
    }

    #[test]
    fn publish_lens_changed_uses_default_lens_when_preference_is_cleared() {
        let runtime = RegistryRuntime::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        runtime.subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
            seen.lock()
                .expect("observer lock poisoned")
                .push(signal.kind.clone());
            Ok(())
        });

        let resolved_id = runtime.publish_lens_changed(None);

        assert_eq!(resolved_id, lens::LENS_ID_DEFAULT);
        let observed = observed.lock().expect("observer lock poisoned");
        assert!(observed.iter().any(|signal| matches!(
            signal,
            SignalKind::RegistryEvent(RegistryEventSignal::LensChanged { new_lens_id })
                if new_lens_id == lens::LENS_ID_DEFAULT
        )));
    }

    #[test]
    fn phase3_workflow_activation_routes_through_lifecycle_signal_observers() {
        let runtime = RegistryRuntime::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let observer_count = Arc::new(AtomicUsize::new(0));

        {
            let observer_count = Arc::clone(&observer_count);
            runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if let SignalKind::Lifecycle(LifecycleSignal::WorkflowActivated { workflow_id }) =
                    &signal.kind
                    && workflow_id == workflow::WORKFLOW_RESEARCH
                {
                    observer_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        let activation = runtime
            .activate_workflow(&mut app, workflow::WORKFLOW_RESEARCH)
            .expect("workflow activation should succeed");

        assert_eq!(activation.workflow_id, workflow::WORKFLOW_RESEARCH);
        assert_eq!(observer_count.load(Ordering::Relaxed), 1);
        assert_eq!(
            runtime.resolve_active_workbench_surface_profile().resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_COMPARE
        );
    }

    #[test]
    fn phase3_semantic_reconcile_routes_through_lifecycle_signal_observers() {
        let runtime = RegistryRuntime::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/math".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(node, "udc:51".to_string());
        app.workspace.graph_runtime.semantic_index_dirty = true;

        let observer_count = Arc::new(AtomicUsize::new(0));
        {
            let observer_count = Arc::clone(&observer_count);
            runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if let SignalKind::Lifecycle(LifecycleSignal::SemanticIndexUpdated {
                    indexed_nodes,
                }) = &signal.kind
                    && *indexed_nodes == 1
                {
                    observer_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });
        }

        let report = runtime.reconcile_semantics(&mut app);
        assert!(report.changed);
        assert_eq!(report.indexed_nodes, 1);
        assert_eq!(observer_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn phase3_knowledge_runtime_exposes_query_and_validation_surface() {
        let runtime = RegistryRuntime::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let math = app.add_node_and_sync(
            "https://example.com/math".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let numerical = app.add_node_and_sync(
            "https://example.com/numerical".to_string(),
            euclid::default::Point2D::new(20.0, 0.0),
        );

        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(math, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(numerical, "udc:519.6".to_string());
        app.workspace.graph_runtime.semantic_index_dirty = true;
        let _ = runtime.reconcile_semantics(&mut app);

        assert_eq!(runtime.query_knowledge_by_tag(&app, "51"), vec![math]);
        assert_eq!(
            runtime.knowledge_tags_for_node(&app, &numerical),
            vec!["udc:519.6".to_string()]
        );
        assert!(matches!(
            runtime.validate_knowledge_tag("519.6"),
            TagValidationResult::Valid { canonical_code, .. } if canonical_code == "519.6"
        ));
        assert_eq!(
            runtime.knowledge_label("5").as_deref(),
            Some("Mathematics and natural sciences")
        );
        assert_eq!(
            runtime.knowledge_color_hint("7"),
            Some(egui::Color32::from_rgb(250, 100, 100))
        );
        assert!(
            runtime
                .semantic_distance("udc:519.6", "51")
                .is_some_and(|distance| distance < 1.0)
        );
        assert_eq!(
            runtime.suggest_semantic_placement_anchor(&app, numerical),
            Some(math)
        );
    }

    #[test]
    fn phase3_subsystem_health_memory_pressure_routes_through_lifecycle_signals() {
        let runtime = RegistryRuntime::default();
        let seen_warning = Arc::new(AtomicUsize::new(0));

        {
            let seen_warning = Arc::clone(&seen_warning);
            runtime.subscribe_signal(SignalTopic::Lifecycle, move |signal| {
                if let SignalKind::Lifecycle(LifecycleSignal::MemoryPressureChanged {
                    level,
                    available_mib,
                    total_mib,
                }) = &signal.kind
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
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_WORKBENCH_SURFACE_PROFILE_ACTIVATED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_CANVAS_PROFILE_ACTIVATED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_PHYSICS_PROFILE_ACTIVATED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_THEME_ACTIVATED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_AGENT_SPAWNED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_AGENT_INTENT_DROPPED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_WORKFLOW_ACTIVATED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INDEX_SEARCH)
        );
    }

    #[test]
    fn diagnostics_registry_declares_system_task_budget_channels() {
        let channels = diagnostics::phase3_required_channels();
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_SYSTEM_TASK_BUDGET_BACKPRESSURE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_SYSTEM_TASK_BUDGET_WORKER_SUSPENDED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_SYSTEM_TASK_BUDGET_WORKER_RESUMED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_SYSTEM_TASK_BUDGET_QUEUE_DEPTH)
        );
        // Verify severities per spec §6
        assert!(channels.iter().any(|entry| {
            entry.channel_id == CHANNEL_SYSTEM_TASK_BUDGET_BACKPRESSURE
                && entry.severity == crate::registries::atomic::diagnostics::ChannelSeverity::Warn
        }));
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
            crate::shell::desktop::runtime::registries::input::binding_id::toolbar::NAV_RELOAD,
        ));
    }

    #[test]
    fn phase2_lens_registry_resolves_default_lens_id() {
        let lens =
            phase2_resolve_lens(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT);
        assert_eq!(lens.display_name, "Default");
        assert_eq!(
            lens.lens_id,
            crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT
        );
        assert_eq!(lens.physics.name, "Drift");
        assert!(matches!(
            lens.layout,
            crate::registries::atomic::lens::LayoutMode::Free
        ));
        assert_eq!(
            lens.layout_algorithm_id,
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED
        );
        assert_eq!(
            lens.theme.as_ref().map(|theme| theme.background_rgb),
            Some((20, 20, 25))
        );
    }

    #[test]
    fn phase2_lens_registry_falls_back_for_unknown_lens_id() {
        let lens = phase2_resolve_lens("lens:unknown");
        assert_eq!(lens.display_name, "Default");
        assert_eq!(
            lens.lens_id,
            crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT
        );
    }

    #[test]
    fn phase2_lens_registry_resolves_semantic_overlay_for_semantic_content() {
        let lens = phase2_resolve_lens_for_content(Some("text/markdown"), true);
        assert_eq!(
            lens.lens_id,
            crate::shell::desktop::runtime::registries::lens::LENS_ID_SEMANTIC_OVERLAY
        );
        assert!(
            lens.filters_legacy
                .iter()
                .any(|filter| filter == "semantic:overlay")
        );
    }

    #[test]
    fn phase2_lens_registry_resolves_semantic_overlay_for_tagged_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync(
            "file:///notes/topic.md".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.apply_reducer_intents([GraphIntent::TagNode {
            key,
            tag: "udc:001".to_string(),
        }]);

        let lens = phase2_resolve_lens_for_node(&app, key);
        assert_eq!(
            lens.lens_id,
            crate::shell::desktop::runtime::registries::lens::LENS_ID_SEMANTIC_OVERLAY
        );
    }

    #[test]
    fn phase2_lens_resolution_preserves_direct_values() {
        let mut lens = crate::app::ResolvedLensPreset {
            lens_id: crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT.to_string(),
            display_name: "Default".to_string(),
            physics: crate::registries::atomic::lens::PhysicsProfile::default(),
            layout: crate::registries::atomic::lens::LayoutMode::Free,
            layout_algorithm_id: crate::app::graph_layout::default_free_layout_algorithm_id(),
            theme: None,
            filter_expr: None,
            filters_legacy: Vec::new(),
            overlay_descriptor: None,
        };
        lens.physics = crate::registries::atomic::lens::PhysicsProfile::scatter();
        lens.layout = crate::registries::atomic::lens::LayoutMode::Grid { gap: 32.0 };
        lens.theme = Some(crate::registries::atomic::lens::ThemeData {
            background_rgb: (1, 2, 3),
            accent_rgb: (4, 5, 6),
            font_scale: 1.2,
            stroke_width: 2.0,
        });

        assert_eq!(lens.physics.name, "Scatter");
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
        let peer_id = crate::mods::native::verse::generate_p2p_secret_key()
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
        let peer_id = crate::mods::native::verse::generate_p2p_secret_key()
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
        assert!(
            intents.is_empty()
                || intents.iter().all(|intent| {
                    matches!(
                        intent,
                        GraphIntent::GrantWorkspaceAccess { workspace_id, .. }
                            if workspace_id == "workspace:test"
                    )
                })
        );
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
    fn phase3_viewer_surface_resolution_returns_web_profile_for_webview() {
        let resolution = phase3_resolve_viewer_surface_profile("viewer:webview");
        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(
            resolution.resolved_id,
            crate::registries::domain::layout::viewer_surface::VIEWER_SURFACE_WEB
        );
    }

    #[test]
    fn phase3_viewer_surface_resolution_returns_document_profile_for_markdown() {
        let resolution = phase3_resolve_viewer_surface_profile("viewer:markdown");
        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(
            resolution.resolved_id,
            crate::registries::domain::layout::viewer_surface::VIEWER_SURFACE_DOCUMENT
        );
    }

    #[test]
    fn phase3_layout_registry_resolves_and_falls_back() {
        let grid =
            phase3_resolve_layout_algorithm(Some(crate::app::graph_layout::GRAPH_LAYOUT_GRID));
        assert_eq!(
            grid.resolved_id,
            crate::app::graph_layout::GRAPH_LAYOUT_GRID
        );
        assert_eq!(grid.capability.display_name, "Grid");

        let fallback = phase3_resolve_layout_algorithm(Some("graph_layout:missing"));
        assert!(fallback.fallback_used);
        assert_eq!(
            fallback.resolved_id,
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED
        );
    }

    #[test]
    fn phase3_layout_registry_applies_grid_positions_to_graph() {
        let mut graph = crate::graph::Graph::new();
        let a = graph.add_node(
            "https://a.test".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = graph.add_node(
            "https://b.test".into(),
            euclid::default::Point2D::new(10.0, 0.0),
        );
        let before_a = graph.node_projected_position(a).unwrap();
        let before_b = graph.node_projected_position(b).unwrap();

        let execution = phase3_apply_layout_algorithm_to_graph(
            &mut graph,
            Some(crate::app::graph_layout::GRAPH_LAYOUT_GRID),
        )
        .expect("grid layout should apply");

        assert_eq!(
            execution.resolution.resolved_id,
            crate::app::graph_layout::GRAPH_LAYOUT_GRID
        );
        assert!(execution.changed_positions > 0);
        assert_ne!(graph.node_projected_position(a).unwrap(), before_a);
        assert_ne!(graph.node_projected_position(b).unwrap(), before_b);
    }

    #[test]
    fn phase3_workbench_surface_resolution_returns_default_profile() {
        // Reset to default first to avoid contamination from tests that change the active profile.
        phase3_set_active_workbench_surface_profile(
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_DEFAULT,
        );
        let resolution = phase3_resolve_active_workbench_surface_profile();
        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(
            resolution.resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_DEFAULT
        );
    }

    #[test]
    fn phase3_canvas_profile_switches_and_applies_workspace_preferences() {
        let switched = phase3_set_active_canvas_profile(
            crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT,
        );
        assert!(switched.matched);
        assert_eq!(
            switched.resolved_id,
            crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT
        );

        let updated = phase3_set_active_canvas_keyboard_pan_step(36.0);
        assert_eq!(updated.profile.navigation.keyboard_pan_step, 36.0);

        let updated = phase3_set_active_canvas_lasso_binding(CanvasLassoBinding::ShiftLeftDrag);
        assert_eq!(
            updated.profile.interaction.lasso_binding,
            CanvasLassoBinding::ShiftLeftDrag
        );
    }

    #[test]
    fn phase3_physics_profile_switches_and_falls_back() {
        let scatter = phase3_set_active_physics_profile(physics_profile::PHYSICS_PROFILE_SCATTER);
        assert!(scatter.matched);
        assert_eq!(
            scatter.resolved_id,
            physics_profile::PHYSICS_PROFILE_SCATTER
        );

        let fallback = phase3_set_active_physics_profile("physics:missing");
        assert!(fallback.fallback_used);
        assert_eq!(
            fallback.resolved_id,
            physics_profile::PHYSICS_PROFILE_DEFAULT
        );
    }

    #[test]
    fn phase3_presentation_profile_tracks_active_physics_and_theme() {
        phase3_set_active_physics_profile(physics_profile::PHYSICS_PROFILE_SETTLE);

        let dark = phase3_resolve_active_presentation_profile(Some(
            crate::registries::atomic::lens::THEME_ID_DARK,
        ));
        assert_eq!(
            dark.physics.resolved_id,
            physics_profile::PHYSICS_PROFILE_SETTLE
        );
        assert_eq!(
            dark.theme.resolved_id,
            crate::registries::atomic::lens::THEME_ID_DARK
        );
        assert_eq!(
            dark.resolved_profile_id,
            crate::registries::domain::presentation::PRESENTATION_PROFILE_DARK
        );

        let fallback = phase3_resolve_active_presentation_profile(Some("theme:missing"));
        assert!(fallback.theme.fallback_used);
        assert_eq!(
            fallback.resolved_profile_id,
            crate::registries::domain::presentation::PRESENTATION_PROFILE_DEFAULT
        );
    }

    #[test]
    fn phase3_workbench_surface_switches_and_describes_profiles() {
        let switched = phase3_set_active_workbench_surface_profile(
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_COMPARE,
        );
        assert!(switched.matched);
        assert_eq!(
            switched.resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_COMPARE
        );

        let description = phase3_describe_workbench_surface(None);
        assert_eq!(description.display_name, "Compare");
        assert_eq!(description.resolved_id, switched.resolved_id);

        let fallback = phase3_set_active_workbench_surface_profile("workbench_surface:missing");
        assert!(fallback.fallback_used);
        assert_eq!(
            fallback.resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_DEFAULT
        );
    }

    #[test]
    fn phase3_workflow_describes_stub_and_activates_runtime_defaults() {
        let capability = phase3_describe_workflow(Some(workflow::WORKFLOW_HISTORY));
        assert_eq!(capability.display_name, "History");
        assert!(!capability.implemented);

        let mut app = GraphBrowserApp::new_for_testing();
        let activation = phase3_activate_workflow(&mut app, workflow::WORKFLOW_READING)
            .expect("workflow activation should succeed");

        assert_eq!(activation.workflow_id, workflow::WORKFLOW_READING);
        assert_eq!(
            phase3_resolve_active_workbench_surface_profile().resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_FOCUS
        );
        assert_eq!(app.default_registry_physics_id(), Some("physics:settle"));
    }

    #[test]
    fn phase3_apply_runtime_action_dispatch_activates_workflow() {
        let mut app = GraphBrowserApp::new_for_testing();
        let dispatch = ActionDispatch {
            intents: Vec::new(),
            workbench_intents: Vec::new(),
            app_commands: Vec::new(),
            runtime_actions: vec![RuntimeAction::ActivateWorkflow {
                workflow_id: workflow::WORKFLOW_RESEARCH.to_string(),
            }],
        };

        let applied =
            phase3_apply_runtime_action_dispatch(&mut app, dispatch).expect("dispatch applies");

        assert_eq!(applied.runtime_actions.len(), 1);
        assert_eq!(
            phase3_resolve_active_workbench_surface_profile().resolved_id,
            crate::shell::desktop::runtime::registries::workbench_surface::WORKBENCH_PROFILE_COMPARE
        );
        assert_eq!(app.default_registry_physics_id(), Some("physics:scatter"));
    }

    #[test]
    fn phase3_apply_runtime_action_dispatch_publishes_settings_route_signal() {
        let mut app = GraphBrowserApp::new_for_testing();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = phase3_subscribe_signal(SignalTopic::RegistryEvent, move |signal| {
            if let SignalKind::RegistryEvent(RegistryEventSignal::SettingsRouteRequested {
                url,
            }) = &signal.kind
            {
                seen.lock()
                    .expect("observer lock poisoned")
                    .push(url.clone());
            }
            Ok(())
        });

        let dispatch = ActionDispatch {
            intents: Vec::new(),
            workbench_intents: Vec::new(),
            app_commands: Vec::new(),
            runtime_actions: vec![RuntimeAction::PublishSettingsRouteRequested {
                url: crate::util::VersoAddress::settings(
                    crate::util::GraphshellSettingsPath::Persistence,
                )
                .to_string(),
            }],
        };

        let applied =
            phase3_apply_runtime_action_dispatch(&mut app, dispatch).expect("dispatch applies");

        assert_eq!(applied.runtime_actions.len(), 1);
        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|route| {
                    route
                        == &crate::util::VersoAddress::settings(
                            crate::util::GraphshellSettingsPath::Persistence,
                        )
                        .to_string()
                })
        );
        assert!(phase3_unsubscribe_signal(
            SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn phase3_execute_registry_action_enqueues_workbench_intent() {
        let mut app = GraphBrowserApp::new_for_testing();

        let intents = phase3_execute_registry_action(
            &mut app,
            action::ACTION_WORKBENCH_OPEN_HISTORY_MANAGER,
            ActionPayload::WorkbenchOpenHistoryManager,
        )
        .expect("history manager action should execute");

        assert!(intents.is_empty());
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::OpenToolPane {
                kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager
            }]
        ));
    }

    #[test]
    fn phase3_nostr_sign_event_returns_signed_payload() {
        let _guard = nostr_backend_test_guard();
        phase3_nostr_use_local_signer();
        let unsigned = NostrUnsignedEvent {
            created_at: 1_710_000_101,
            kind: 1,
            content: "hello".to_string(),
            tags: vec![vec!["t".to_string(), "graphshell".to_string()]],
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
                created_at: 1_710_000_102,
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
            created_at: 1_710_000_103,
            kind: 1,
            content: "bad".to_string(),
            tags: Vec::new(),
        });

        assert!(publish.is_err());
    }

    #[test]
    fn phase3_nostr_nip46_backend_reports_unavailable() {
        let _guard = nostr_backend_test_guard();
        let signer_secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        let signer_keypair =
            secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &signer_secret);
        let (signer_pubkey, _) = secp256k1::XOnlyPublicKey::from_keypair(&signer_keypair);
        phase3_nostr_use_nip46_signer("wss://relay.example", &signer_pubkey.to_string())
            .expect("nip46 config should be accepted");
        phase3_nostr_set_nip46_permission("sign_event", Nip46PermissionDecision::Allow)
            .expect("nip46 permission should be stored");

        let result = phase3_nostr_sign_event(
            "default",
            &NostrUnsignedEvent {
                created_at: 1_710_000_104,
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

    #[test]
    fn phase3_shared_runtime_returns_single_global_authority() {
        let a = phase3_shared_runtime();
        let b = phase3_shared_runtime();

        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn registry_runtime_describes_builtin_tag_suggester_agent() {
        let runtime = phase3_shared_runtime();
        let descriptor = runtime
            .describe_agent("agent:tag_suggester")
            .expect("tag suggester descriptor should exist");

        assert_eq!(descriptor.id, "agent:tag_suggester");
        assert!(
            descriptor
                .capabilities
                .contains(&agent::AgentCapability::SuggestNodeTags)
        );
    }
}
